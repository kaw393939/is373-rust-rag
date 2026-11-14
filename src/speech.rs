//! Local speech adapter. No audio leaves this process and its local child processes.
use crate::{
    local_process::{executable_exists, run},
    providers::{NoSpeech, SpeechStatus, Transcriber, Transcript},
};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::process::Command;

pub fn configured() -> Result<Arc<dyn Transcriber>> {
    let provider = std::env::var("TRANSCRIPTION_PROVIDER").unwrap_or_else(|_| "whisper_cpp".into());
    if provider == "groq" {
        return Ok(Arc::new(crate::groq_speech::GroqSpeech::configured()?));
    }
    if provider != "whisper_cpp" {
        bail!("TRANSCRIPTION_PROVIDER={provider}: adapter not installed");
    }
    let model = PathBuf::from(
        std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "data/models/ggml-small.bin".into()),
    );
    let language = std::env::var("WHISPER_LANGUAGE").unwrap_or_else(|_| "auto".into());
    let revision = if model.is_file() {
        let mut file = std::fs::File::open(&model)?;
        let mut digest = Sha256::new();
        std::io::copy(&mut file, &mut digest)?;
        format!("{:x}", digest.finalize())
    } else {
        "missing".into()
    };
    Ok(Arc::new(Whisper {
        cli: std::env::var("WHISPER_CLI").unwrap_or_else(|_| "whisper-cli".into()),
        ffmpeg: std::env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".into()),
        key: format!("whisper_cpp:{revision}:{language}:pcm16-v1"),
        model,
        revision,
        language,
    }))
}

struct Whisper {
    cli: String,
    ffmpeg: String,
    model: PathBuf,
    language: String,
    revision: String,
    key: String,
}

fn audio_measurements(path: &Path) -> Result<f64> {
    let mut reader = hound::WavReader::open(path).context("Could not decode audio")?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16 {
        bail!("Unexpected audio conversion format");
    }
    let duration = reader.duration() as f64 / 16000.0;
    if !(0.2..=120.0).contains(&duration) {
        bail!("Audio must be between 0.2 seconds and 2 minutes. Trim the recording and try again.");
    }
    let mut power = 0f64;
    for sample in reader.samples::<i16>() {
        let x = sample? as f64 / 32768.0;
        power += x * x;
    }
    if (power / (duration * 16000.0)).sqrt() < 0.0005 {
        return Err(NoSpeech.into());
    }
    Ok(duration)
}

pub async fn normalize(
    bytes: Vec<u8>,
    mime: &str,
    ffmpeg: &str,
) -> Result<(tempfile::TempDir, PathBuf, f64)> {
    if !matches!(
        mime,
        "audio/wav" | "audio/mpeg" | "audio/ogg" | "audio/mp4" | "audio/webm" | "audio/flac"
    ) {
        bail!("Unsupported audio format");
    }
    let temp = tempfile::Builder::new().prefix("ordo-speech-").tempdir()?;
    let input = temp.path().join("input.media");
    let wav = temp.path().join("speech.wav");
    tokio::fs::write(&input, bytes).await?;
    let mut convert = Command::new(ffmpeg);
    convert
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(&input)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-t",
            "121",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            "-y",
        ])
        .arg(&wav);
    run(convert, 30, "Audio decoding").await?;
    let measure_path = wav.clone();
    let duration = tokio::task::spawn_blocking(move || audio_measurements(&measure_path)).await??;
    Ok((temp, wav, duration))
}

#[async_trait]
impl Transcriber for Whisper {
    fn is_local(&self) -> bool {
        true
    }
    fn cache_key(&self) -> &str {
        &self.key
    }
    async fn status(&self) -> SpeechStatus {
        let ready = self.revision != "missing"
            && self.model.is_file()
            && executable_exists(&self.cli)
            && executable_exists(&self.ffmpeg);
        SpeechStatus { ready, provider: "whisper_cpp".into(), model: self.model.file_name().unwrap_or_default().to_string_lossy().into_owned(), language: self.language.clone(), max_seconds: 120,
            message: (!ready).then(|| "Install whisper-cpp and FFmpeg, download the Whisper model, then restart the server. See README.".into()) }
    }
    async fn transcribe(&self, bytes: Vec<u8>, mime: &str) -> Result<Transcript> {
        let status = self.status().await;
        if !status.ready {
            bail!("{}", status.message.unwrap_or_default());
        }
        let (temp, wav, duration) = normalize(bytes, mime, &self.ffmpeg).await?;
        let output = temp.path().join("transcript");
        let mut whisper = Command::new(&self.cli);
        whisper
            .arg("-m")
            .arg(&self.model)
            .arg("-f")
            .arg(&wav)
            .arg("-l")
            .arg(&self.language)
            .args(["-oj", "-np", "-t", "4"])
            .arg("-of")
            .arg(&output);
        run(whisper, 180, "Local transcription").await?;
        let json_path = output.with_extension("json");
        if tokio::fs::metadata(&json_path).await?.len() > 2_000_000 {
            bail!("Transcription output exceeds limit");
        }
        let result: Value = serde_json::from_slice(&tokio::fs::read(json_path).await?)?;
        let segments = result["transcription"]
            .as_array()
            .context("Invalid transcription output")?;
        let text = segments
            .iter()
            .filter_map(|s| s["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return Err(NoSpeech.into());
        }
        if text.chars().count() > 16000 {
            bail!("Transcript exceeds the context limit. Use a shorter recording.");
        }
        Ok(Transcript {
            text,
            language: result["result"]["language"]
                .as_str()
                .unwrap_or(&self.language)
                .into(),
            duration_seconds: duration,
            provider: "whisper_cpp".into(),
            model: status.model,
            revision: self.revision.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_silence_and_overlong_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..16000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        assert!(
            audio_measurements(&path)
                .unwrap_err()
                .to_string()
                .contains("No audible")
        );
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..16000 * 121 {
            writer.write_sample(1000i16).unwrap();
        }
        writer.finalize().unwrap();
        assert!(
            audio_measurements(&path)
                .unwrap_err()
                .to_string()
                .contains("2 minutes")
        );
    }
}

//! Groq speech recognition shares local bounded decoding with whisper.cpp.
use crate::{
    groq::Groq,
    local_process::executable_exists,
    providers::{NoSpeech, SpeechStatus, Transcriber, Transcript},
};
use anyhow::{Result, bail};
use async_trait::async_trait;
use serde_json::Value;
pub struct GroqSpeech {
    client: reqwest::Client,
    auth: Groq,
    model: String,
    language: String,
    ffmpeg: String,
    key: String,
}
impl GroqSpeech {
    pub fn configured() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("is373-rust-rag/0.1")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(180))
            .build()?;
        let auth = Groq::configured(client.clone())?;
        let model = std::env::var("TRANSCRIPTION_MODEL")
            .unwrap_or_else(|_| "whisper-large-v3-turbo".into());
        if !["whisper-large-v3", "whisper-large-v3-turbo"].contains(&model.as_str()) {
            bail!("Unsupported Groq speech model");
        }
        let language = std::env::var("WHISPER_LANGUAGE").unwrap_or_else(|_| "auto".into());
        Ok(Self {
            client,
            auth,
            key: format!("groq:{model}:{language}:pcm16-v1"),
            model,
            language,
            ffmpeg: std::env::var("FFMPEG_BIN").unwrap_or_else(|_| "ffmpeg".into()),
        })
    }
}
#[async_trait]
impl Transcriber for GroqSpeech {
    fn cache_key(&self) -> &str {
        &self.key
    }
    async fn status(&self) -> SpeechStatus {
        let ready = self.auth.key().is_ok() && executable_exists(&self.ffmpeg);
        SpeechStatus {
            ready,
            provider: "groq".into(),
            model: self.model.clone(),
            language: self.language.clone(),
            max_seconds: 120,
            message: (!ready).then(|| "Configure the server Groq key and FFmpeg.".into()),
        }
    }
    async fn transcribe(&self, bytes: Vec<u8>, mime: &str) -> Result<Transcript> {
        let (_temp, wav, duration) = crate::speech::normalize(bytes, mime, &self.ffmpeg).await?;
        let part = reqwest::multipart::Part::bytes(tokio::fs::read(wav).await?)
            .file_name("speech.wav")
            .mime_str("audio/wav")?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");
        if self.language != "auto" {
            form = form.text("language", self.language.clone());
        }
        let response = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(self.auth.key()?)
            .multipart(form)
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Groq transcription returned HTTP {}", response.status());
        }
        let result: Value = response.json().await?;
        let text = result["text"].as_str().unwrap_or("").trim().to_owned();
        if text.is_empty() {
            return Err(NoSpeech.into());
        }
        if text.chars().count() > 16000 {
            bail!("Transcript exceeds context limit");
        }
        Ok(Transcript {
            text,
            language: result["language"].as_str().unwrap_or("unknown").into(),
            duration_seconds: duration,
            provider: "groq".into(),
            model: self.model.clone(),
            revision: self.model.clone(),
        })
    }
}

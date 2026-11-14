//! Provider-neutral document/video decoding. These adapters do not answer questions.
use crate::{
    local_process::{executable_exists, output, run},
    providers::ImageInput,
};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::{path::Path, sync::Arc};
use tokio::process::Command;

pub const MAX_PDF_PAGES: usize = 20;
pub const MAX_PDF_TEXT: usize = 16000;
pub const MAX_SCAN_PAGES: usize = 4;
pub const MAX_VIDEO_SECONDS: f64 = 120.0;

#[derive(Serialize)]
pub struct ProcessorStatus {
    pub ready: bool,
    pub provider: String,
    pub message: Option<String>,
}
pub struct ExtractedPage {
    pub page: usize,
    pub text: String,
    pub image: Option<ImageInput>,
}
pub struct ExtractedDocument {
    pub pages: Vec<ExtractedPage>,
    pub warnings: Vec<String>,
}
pub struct DecodedFrame {
    pub timestamp_seconds: f64,
    pub image: ImageInput,
}
pub struct DecodedVideo {
    pub duration_seconds: f64,
    pub frames: Vec<DecodedFrame>,
    pub audio: Option<Vec<u8>>,
    pub warnings: Vec<String>,
}

#[async_trait]
pub trait DocumentExtractor: Send + Sync {
    fn is_local(&self) -> bool {
        false
    }
    fn cache_key(&self) -> &str;
    fn status(&self) -> ProcessorStatus;
    async fn extract(&self, bytes: Vec<u8>) -> Result<ExtractedDocument>;
}
#[async_trait]
pub trait VideoDecoder: Send + Sync {
    fn is_local(&self) -> bool {
        false
    }
    fn cache_key(&self) -> &str;
    fn status(&self) -> ProcessorStatus;
    async fn decode(&self, bytes: Vec<u8>) -> Result<DecodedVideo>;
}
#[derive(Clone)]
pub struct MediaProcessors {
    pub pdf: Arc<dyn DocumentExtractor>,
    pub video: Arc<dyn VideoDecoder>,
}
pub fn configured() -> Result<MediaProcessors> {
    for (key, expected) in [("PDF_PROVIDER", "poppler"), ("VIDEO_PROVIDER", "ffmpeg")] {
        let value = std::env::var(key).unwrap_or_else(|_| expected.into());
        if value != expected {
            bail!("{key}={value}: adapter not installed; see docs/multimedia.md");
        }
    }
    let env = |key, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
    let info = env("PDFINFO_BIN", "pdfinfo");
    let text = env("PDFTOTEXT_BIN", "pdftotext");
    let render = env("PDFTOPPM_BIN", "pdftoppm");
    let ffmpeg = env("FFMPEG_BIN", "ffmpeg");
    let ffprobe = env("FFPROBE_BIN", "ffprobe");
    let frame_count: usize = env("VIDEO_SAMPLE_FRAMES", "3")
        .parse()
        .context("VIDEO_SAMPLE_FRAMES must be 1 through 4")?;
    if !(1..=4).contains(&frame_count) {
        bail!("VIDEO_SAMPLE_FRAMES must be 1 through 4");
    }
    Ok(MediaProcessors {
        pdf: Arc::new(Poppler {
            key: format!("poppler-v1:{info}:{text}:{render}:20p:16000c:4scan:1280px"),
            info,
            text,
            render,
        }),
        video: Arc::new(Ffmpeg {
            key: format!("ffmpeg-v1:{ffmpeg}:{ffprobe}:{frame_count}:midpoint:768px:120s"),
            ffmpeg,
            ffprobe,
            frame_count,
        }),
    })
}

struct Poppler {
    info: String,
    text: String,
    render: String,
    key: String,
}
fn pdf_page_count(info: &str) -> Result<usize> {
    if info
        .lines()
        .any(|l| l.starts_with("Encrypted:") && l.split_whitespace().nth(1) != Some("no"))
    {
        bail!(
            "Encrypted PDFs are not supported. Upload an unlocked copy you are permitted to read."
        );
    }
    let count = info
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .context("Could not determine PDF page count")?
        .trim()
        .parse::<usize>()?;
    if count == 0 || count > MAX_PDF_PAGES {
        bail!("PDFs are limited to 20 pages. Split the document and try again.");
    }
    Ok(count)
}
fn pdf_pages(text: &str, count: usize) -> Result<Vec<String>> {
    let mut pages = text
        .split('\u{c}')
        .map(|p| p.trim().to_owned())
        .collect::<Vec<_>>();
    if pages.last().is_some_and(|p| p.is_empty()) && pages.len() == count + 1 {
        pages.pop();
    }
    if pages.len() != count {
        bail!("PDF extraction could not preserve page boundaries");
    }
    if pages.iter().map(|p| p.chars().count()).sum::<usize>() > MAX_PDF_TEXT {
        bail!(
            "PDF text exceeds 16,000 characters. Split the document or upload a shorter excerpt."
        );
    }
    if pages
        .iter()
        .filter(|p| p.chars().filter(|c| c.is_alphanumeric()).count() < 8)
        .count()
        > MAX_SCAN_PAGES
    {
        bail!(
            "This PDF needs more than four visual pages. Split the scanned pages into smaller PDFs."
        );
    }
    Ok(pages)
}

#[async_trait]
impl DocumentExtractor for Poppler {
    fn is_local(&self) -> bool {
        true
    }
    fn cache_key(&self) -> &str {
        &self.key
    }
    fn status(&self) -> ProcessorStatus {
        let ready = [&self.info, &self.text, &self.render]
            .iter()
            .all(|b| executable_exists(b));
        ProcessorStatus { ready,provider:"poppler".into(),message:(!ready).then(||"Install Poppler (pdfinfo, pdftotext, pdftoppm) and restart the server. See README.".into()) }
    }
    async fn extract(&self, bytes: Vec<u8>) -> Result<ExtractedDocument> {
        if !self.status().ready {
            bail!("{}", self.status().message.unwrap_or_default());
        }
        let temp = tempfile::Builder::new().prefix("ordo-pdf-").tempdir()?;
        let input = temp.path().join("input.pdf");
        tokio::fs::write(&input, bytes).await?;
        let mut info = Command::new(&self.info);
        info.env("LC_ALL", "C").arg(&input);
        let info = String::from_utf8(output(info, 15, 64000, "PDF inspection").await?)?;
        let count = pdf_page_count(&info)?;
        let mut extract = Command::new(&self.text);
        extract
            .args(["-layout", "-enc", "UTF-8"])
            .arg(&input)
            .arg("-");
        let text = String::from_utf8(output(extract, 30, 100000, "PDF text extraction").await?)?;
        let text_pages = pdf_pages(&text, count)?;
        let mut pages = Vec::new();
        let mut warnings=vec!["Text-layer pages are extracted as text. Figures and complex table layout may not be represented.".into()];
        for (i, text) in text_pages.into_iter().enumerate() {
            let page = i + 1;
            let image = if text.chars().filter(|c| c.is_alphanumeric()).count() < 8 {
                let prefix = temp.path().join(format!("page-{page}"));
                let mut render = Command::new(&self.render);
                render
                    .args([
                        "-f",
                        &page.to_string(),
                        "-l",
                        &page.to_string(),
                        "-singlefile",
                        "-scale-to",
                        "1280",
                        "-jpeg",
                        "-jpegopt",
                        "quality=85",
                    ])
                    .arg(&input)
                    .arg(&prefix);
                run(render, 30, "PDF page rendering").await?;
                let bytes = bounded_read(&prefix.with_extension("jpg"), 2_000_000).await?;
                warnings.push(format!("Page {page} has little or no text layer; a rendered page is supplied to the vision model. This is not a saved OCR transcript."));
                Some(ImageInput {
                    mime: "image/jpeg".into(),
                    bytes,
                    label: String::new(),
                })
            } else {
                None
            };
            pages.push(ExtractedPage { page, text, image });
        }
        Ok(ExtractedDocument { pages, warnings })
    }
}

pub async fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if tokio::fs::metadata(path).await?.len() > limit {
        bail!("Processed media exceeds the output limit");
    }
    Ok(tokio::fs::read(path).await?)
}

struct Ffmpeg {
    ffmpeg: String,
    ffprobe: String,
    frame_count: usize,
    key: String,
}
struct VideoInfo {
    duration: Option<f64>,
    video: u64,
    audio: Option<u64>,
    warnings: Vec<String>,
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .filter(|n| n.is_finite() && *n > 0.0)
}
fn video_info(value: &Value) -> Result<VideoInfo> {
    let streams = value["streams"]
        .as_array()
        .context("Invalid video metadata")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1)
        .context("No video stream found. Upload audio-only recordings as audio files.")?;
    let pixels = video["width"]
        .as_u64()
        .unwrap_or(0)
        .saturating_mul(video["height"].as_u64().unwrap_or(0));
    if pixels == 0 || pixels > 20_000_000 {
        bail!("Video frames must be at most 20 megapixels");
    }
    let duration = std::iter::once(&value["format"]["duration"])
        .chain(streams.iter().map(|s| &s["duration"]))
        .filter_map(number)
        .reduce(f64::max);
    if duration.is_some_and(|d| d > MAX_VIDEO_SECONDS) {
        bail!("Videos are limited to 2 minutes. Trim the clip and try again.");
    }
    let audio = streams
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .and_then(|s| s["index"].as_u64());
    let mut warnings=vec!["Only sampled still frames are supplied; motion and events between samples may be missed. Timestamps indicate approximate sample positions.".into()];
    if streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .count()
        > 1
    {
        warnings.push("Only the first audio track is transcribed.".into());
    }
    Ok(VideoInfo {
        duration,
        video: video["index"]
            .as_u64()
            .context("Video stream index missing")?,
        audio,
        warnings,
    })
}
fn sample_times(duration: f64, count: usize) -> Vec<f64> {
    if duration < 0.5 {
        return vec![0.0];
    }
    (0..count)
        .map(|i| (i as f64 + 0.5) * duration / count as f64)
        .collect()
}
#[async_trait]
impl VideoDecoder for Ffmpeg {
    fn is_local(&self) -> bool {
        true
    }
    fn cache_key(&self) -> &str {
        &self.key
    }
    fn status(&self) -> ProcessorStatus {
        let ready = executable_exists(&self.ffmpeg) && executable_exists(&self.ffprobe);
        ProcessorStatus {
            ready,
            provider: "ffmpeg".into(),
            message: (!ready)
                .then(|| "Install FFmpeg and ffprobe, then restart the server. See README.".into()),
        }
    }
    async fn decode(&self, bytes: Vec<u8>) -> Result<DecodedVideo> {
        if !self.status().ready {
            bail!("{}", self.status().message.unwrap_or_default());
        }
        let temp = tempfile::Builder::new().prefix("ordo-video-").tempdir()?;
        let input = temp.path().join("input.video");
        tokio::fs::write(&input, bytes).await?;
        let mut probe = Command::new(&self.ffprobe);
        probe.args(["-v","error","-protocol_whitelist","file,pipe","-show_entries","format=duration:stream=index,codec_type,width,height,duration:stream_disposition=attached_pic","-of","json"]).arg(&input);
        let value: Value =
            serde_json::from_slice(&output(probe, 20, 128000, "Video inspection").await?)?;
        let info = video_info(&value)?;
        let duration = if let Some(d) = info.duration {
            d
        } else {
            // Browser-recorded WebM often omits duration. Inspect bounded packet timestamps.
            let mut probe = Command::new(&self.ffprobe);
            probe
                .args([
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file,pipe",
                    "-select_streams",
                    &info.video.to_string(),
                    "-show_entries",
                    "packet=pts_time,duration_time",
                    "-of",
                    "json",
                ])
                .arg(&input);
            let packets: Value = serde_json::from_slice(
                &output(probe, 20, 2_000_000, "Video duration inspection").await?,
            )?;
            let packets = packets["packets"]
                .as_array()
                .context("Video duration unavailable")?;
            let end = packets
                .iter()
                .filter_map(|p| {
                    p["pts_time"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|t| t + number(&p["duration_time"]).unwrap_or(0.0))
                })
                .filter(|v| v.is_finite())
                .reduce(f64::max)
                .context("Video duration unavailable")?;
            if end <= 0.0 || end > MAX_VIDEO_SECONDS {
                bail!("Video duration must be greater than zero and at most 2 minutes");
            }
            end
        };
        let mut frames = vec![];
        for (i, timestamp_seconds) in sample_times(duration, self.frame_count)
            .into_iter()
            .enumerate()
        {
            let path = temp.path().join(format!("frame-{i}.jpg"));
            let mut command = Command::new(&self.ffmpeg);
            command
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-nostdin",
                    "-protocol_whitelist",
                    "file,pipe",
                    "-ss",
                    &format!("{timestamp_seconds:.3}"),
                    "-i",
                ])
                .arg(&input)
                .args([
                    "-map",
                    &format!("0:{}", info.video),
                    "-frames:v",
                    "1",
                    "-vf",
                    "scale='min(768,iw)':'min(768,ih)':force_original_aspect_ratio=decrease",
                    "-q:v",
                    "3",
                    "-y",
                ])
                .arg(&path);
            run(command, 30, "Video frame decoding").await?;
            frames.push(DecodedFrame {
                timestamp_seconds,
                image: ImageInput {
                    mime: "image/jpeg".into(),
                    bytes: bounded_read(&path, 2_000_000).await?,
                    label: String::new(),
                },
            });
        }
        let audio = if let Some(index) = info.audio {
            let wav = temp.path().join("audio.wav");
            let mut command = Command::new(&self.ffmpeg);
            command
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
                    &format!("0:{index}"),
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
            run(command, 30, "Video audio extraction").await?;
            Some(bounded_read(&wav, 4_000_000).await?)
        } else {
            None
        };
        Ok(DecodedVideo {
            duration_seconds: duration,
            frames,
            audio,
            warnings: info.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    #[ignore = "requires local Poppler/FFmpeg and generated fixtures; no model inference"]
    async fn real_document_and_video_decoding() {
        let p = configured().unwrap();
        let doc = p
            .pdf
            .extract(std::fs::read("tmp/pdfs/workshop.pdf").unwrap())
            .await
            .unwrap();
        assert_eq!(doc.pages.len(), 2);
        assert!(doc.pages[1].text.contains("orange notebook"));
        let scan = p
            .pdf
            .extract(std::fs::read("tmp/pdfs/scanned.pdf").unwrap())
            .await
            .unwrap();
        assert!(scan.pages[0].text.is_empty() && scan.pages[0].image.is_some());
        for file in ["silent.mp4", "browser.webm", "narrated.mp4"] {
            let clip = p
                .video
                .decode(std::fs::read(format!("data/qa/compound/{file}")).unwrap())
                .await
                .unwrap();
            assert!((clip.duration_seconds - 9.0).abs() < 0.2);
            assert_eq!(clip.frames.len(), 3);
            assert_eq!(clip.audio.is_some(), file == "narrated.mp4");
            let image = image::load_from_memory(&clip.frames[1].image.bytes)
                .unwrap()
                .to_rgb8();
            let pixel = image.get_pixel(200, 100);
            assert!(pixel[2] > pixel[0] + 80, "middle frame should be blue");
        }
    }
    #[test]
    fn pdf_limits_encryption_and_page_boundaries() {
        assert!(pdf_page_count("Pages: 21\nEncrypted: no").is_err());
        assert!(pdf_page_count("Pages: 1\nEncrypted: yes (print:yes)").is_err());
        assert_eq!(
            pdf_pages("Page one text\u{c}Page two text\u{c}", 2).unwrap(),
            vec!["Page one text", "Page two text"]
        );
        assert!(pdf_pages("missing separator", 2).is_err());
        assert!(pdf_pages(&"x".repeat(MAX_PDF_TEXT + 1), 1).is_err());
    }
    #[test]
    fn video_sampling_and_metadata_are_bounded() {
        assert_eq!(sample_times(9.0, 3), vec![1.5, 4.5, 7.5]);
        let mut value = json!({"format":{"duration":"9"},"streams":[{"index":0,"codec_type":"video","width":640,"height":360},{"index":1,"codec_type":"audio"}]});
        assert_eq!(video_info(&value).unwrap().audio, Some(1));
        value["format"]["duration"] = json!("121");
        assert!(video_info(&value).is_err());
        value["streams"] = json!([{ "codec_type":"audio" }]);
        assert!(video_info(&value).is_err());
    }
}

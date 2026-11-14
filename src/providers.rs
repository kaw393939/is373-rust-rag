//! Provider-neutral contracts. HTTP handlers never consume vendor response shapes.
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{pin::Pin, sync::Arc, time::Duration};
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    io::StreamReader,
};

#[derive(Clone)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
    pub images: Vec<ImageInput>,
}

/// Owned, validated bytes. Vendor encodings belong exclusively to adapters.
#[derive(Clone)]
pub struct ImageInput {
    pub mime: String,
    pub bytes: Vec<u8>,
    pub label: String,
}

#[derive(Debug)]
pub struct NoSpeech;
impl std::fmt::Display for NoSpeech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("No audible speech was recognized. Try a clearer recording.")
    }
}
impl std::error::Error for NoSpeech {}

#[derive(Clone, Default, Serialize)]
pub struct Capabilities {
    pub text: bool,
    pub image_input: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub language: String,
    pub duration_seconds: f64,
    pub provider: String,
    pub model: String,
    pub revision: String,
}

#[derive(Serialize)]
pub struct SpeechStatus {
    pub ready: bool,
    pub provider: String,
    pub model: String,
    pub language: String,
    pub max_seconds: u64,
    pub message: Option<String>,
}

#[async_trait]
pub trait Transcriber: Send + Sync {
    fn is_local(&self) -> bool {
        false
    }
    fn cache_key(&self) -> &str;
    async fn status(&self) -> SpeechStatus;
    async fn transcribe(&self, bytes: Vec<u8>, mime: &str) -> Result<Transcript>;
}

pub type TokenStream = Pin<Box<dyn Stream<Item = Result<GenerationEvent>> + Send>>;

#[derive(Debug, PartialEq)]
pub enum GenerationEvent {
    Delta(String),
    Finished { complete: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingIdentity {
    pub provider: String,
    pub model: String,
    pub revision: String,
}

#[async_trait]
pub trait Embedder: Send + Sync {
    fn is_local(&self) -> bool {
        false
    }
    async fn identity(&self) -> Result<EmbeddingIdentity>;
    async fn embed(&self, texts: Vec<String>, query: bool) -> Result<Vec<Vec<f32>>>;
}

#[async_trait]
pub trait Generator: Send + Sync {
    fn is_local(&self) -> bool {
        false
    }
    fn model(&self) -> &str;
    fn provider(&self) -> &str;
    async fn capabilities(&self) -> Result<Capabilities>;
    async fn generate(
        &self,
        system: String,
        prompt: ChatTurn,
        history: Vec<ChatTurn>,
    ) -> Result<TokenStream>;
}

pub struct Providers {
    pub embedder: Arc<dyn Embedder>,
    pub generator: Arc<dyn Generator>,
    pub vision: Arc<dyn Generator>,
}

pub fn configured() -> Result<Providers> {
    for key in ["EMBEDDING_PROVIDER", "VISION_PROVIDER"] {
        let value = std::env::var(key).unwrap_or_else(|_| "ollama".into());
        if value != "ollama" {
            bail!("{key}={value}: adapter not installed; see docs/providers.md");
        }
    }
    let chat_provider = std::env::var("CHAT_PROVIDER").unwrap_or_else(|_| "ollama".into());
    let base = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let url = reqwest::Url::parse(&base)?;
    if url.scheme() != "http"
        || !matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "ollama" | "host.docker.internal")
        )
    {
        bail!(
            "OLLAMA_URL must use HTTP with loopback, ollama, or host.docker.internal (the local Docker services)"
        );
    }
    let client = reqwest::Client::builder()
        .user_agent("is373-rust-rag/0.1")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(300))
        .build()?;
    let make = |key: &str, default: &str| Ollama {
        base: base.trim_end_matches('/').into(),
        client: client.clone(),
        model: std::env::var(key).unwrap_or_else(|_| default.into()),
    };
    let generator: Arc<dyn Generator> = match chat_provider.as_str() {
        "ollama" => Arc::new(make("CHAT_MODEL", "gemma3:4b")),
        "groq" => Arc::new(crate::groq::Groq::configured(client.clone())?),
        _ => bail!("CHAT_PROVIDER={chat_provider}: adapter not installed; use ollama or groq"),
    };
    Ok(Providers {
        embedder: Arc::new(make("EMBEDDING_MODEL", "nomic-embed-text:latest")),
        generator,
        vision: Arc::new(make("VISION_MODEL", "gemma3:4b")),
    })
}

struct Ollama {
    base: String,
    client: reqwest::Client,
    model: String,
}

#[async_trait]
impl Embedder for Ollama {
    fn is_local(&self) -> bool {
        true
    }
    async fn identity(&self) -> Result<EmbeddingIdentity> {
        let tags: Value = self
            .client
            .get(format!("{}/api/tags", self.base))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let name = if self.model.contains(':') {
            self.model.clone()
        } else {
            format!("{}:latest", self.model)
        };
        let revision = tags["models"]
            .as_array()
            .context("Invalid model list")?
            .iter()
            .find(|m| m["name"] == name)
            .and_then(|m| m["digest"].as_str())
            .context("Embedding model missing. Run ollama pull nomic-embed-text")?;
        Ok(EmbeddingIdentity {
            provider: "ollama".into(),
            model: self.model.clone(),
            revision: revision.into(),
        })
    }
    async fn embed(&self, texts: Vec<String>, query: bool) -> Result<Vec<Vec<f32>>> {
        // Nomic's retrieval prefixes belong to this adapter, never the retrieval layer.
        let input: Vec<String> = texts
            .into_iter()
            .map(|t| {
                if self.model.starts_with("nomic-embed-text") {
                    format!(
                        "{}: {t}",
                        if query {
                            "search_query"
                        } else {
                            "search_document"
                        }
                    )
                } else {
                    t
                }
            })
            .collect();
        let count = input.len();
        let result: Value = self
            .client
            .post(format!("{}/api/embed", self.base))
            .json(&json!({"model":self.model,"input":input,"truncate":false}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let vectors: Vec<Vec<f32>> = serde_json::from_value(result["embeddings"].clone())?;
        if vectors.len() != count
            || vectors.iter().any(|v| {
                v.is_empty() || v.iter().any(|x| !x.is_finite()) || v.iter().all(|x| *x == 0.0)
            })
        {
            bail!("Invalid embeddings returned by provider");
        }
        Ok(vectors)
    }
}

fn ollama_message(turn: ChatTurn) -> Result<Value> {
    if turn.images.iter().any(|image| {
        !matches!(
            image.mime.as_str(),
            "image/jpeg" | "image/png" | "image/webp"
        )
    }) {
        bail!("Unsupported image encoding for Ollama");
    }
    let images = turn
        .images
        .iter()
        .map(|image| STANDARD.encode(&image.bytes))
        .collect::<Vec<_>>();
    let labels = turn
        .images
        .iter()
        .enumerate()
        .filter(|(_, i)| !i.label.is_empty())
        .map(|(n, i)| format!("Image {}: {}", n + 1, i.label))
        .collect::<Vec<_>>()
        .join("\n");
    let content = if labels.is_empty() {
        turn.content
    } else {
        format!(
            "{}\n\nATTACHED IMAGES IN ORDER (reference labels):\n{labels}",
            turn.content
        )
    };
    let mut message = json!({"role":turn.role,"content":content});
    if !images.is_empty() {
        message["images"] = json!(images);
    }
    Ok(message)
}

fn parse_line(line: &str) -> Result<Vec<GenerationEvent>> {
    let value: Value = serde_json::from_str(line).context("Malformed generation stream")?;
    if value.get("error").is_some() {
        bail!("Local generation failed; check Ollama and installed model");
    }
    let mut events = Vec::new();
    if let Some(s) = value["message"]["content"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        events.push(GenerationEvent::Delta(s.into()));
    }
    if value["done"] == true {
        events.push(GenerationEvent::Finished {
            complete: value["done_reason"] == "stop",
        });
    }
    Ok(events)
}

#[async_trait]
impl Generator for Ollama {
    fn is_local(&self) -> bool {
        true
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn provider(&self) -> &str {
        "ollama"
    }
    async fn capabilities(&self) -> Result<Capabilities> {
        let value: Value = self
            .client
            .post(format!("{}/api/show", self.base))
            .json(&json!({"model": self.model}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let caps = value["capabilities"]
            .as_array()
            .context("Model capabilities unavailable; update Ollama")?;
        Ok(Capabilities {
            text: caps.iter().any(|v| v == "completion"),
            image_input: caps.iter().any(|v| v == "vision"),
        })
    }
    async fn generate(
        &self,
        system: String,
        prompt: ChatTurn,
        history: Vec<ChatTurn>,
    ) -> Result<TokenStream> {
        let mut messages = vec![ChatTurn {
            role: "system".into(),
            content: system,
            images: vec![],
        }];
        messages.extend(history);
        messages.push(prompt);
        let messages = messages
            .into_iter()
            .map(ollama_message)
            .collect::<Result<Vec<_>>>()?;
        let response = self
            .client
            .post(format!("{}/api/chat", self.base))
            .json(
                &json!({"model":self.model,"stream":true,"messages":messages,
                "options":{"temperature":0,"num_predict":2048,"num_ctx":16384}}),
            )
            .send()
            .await?
            .error_for_status()?;
        let reader = StreamReader::new(response.bytes_stream().map_err(std::io::Error::other));
        let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(65536));
        Ok(Box::pin(async_stream::try_stream! {
            while let Some(line) = lines.next().await {
                let line = line?;
                if line.trim().is_empty() { continue; }
                for event in parse_line(&line)? {
                    let done = matches!(event, GenerationEvent::Finished { .. });
                    yield event;
                    if done { return; }
                }
            }
            Err(anyhow::anyhow!("Generation stream ended without a terminal event"))?;
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multimodal_mapping_is_owned_by_adapter() {
        let turn = ChatTurn {
            role: "user".into(),
            content: "Describe it".into(),
            images: vec![ImageInput {
                mime: "image/jpeg".into(),
                label: String::new(),
                bytes: vec![1, 2, 3],
            }],
        };
        let request = ollama_message(turn).unwrap();
        assert_eq!(request["content"], "Describe it");
        assert_eq!(request["images"][0], "AQID");
        assert!(
            ollama_message(ChatTurn {
                role: "user".into(),
                content: "".into(),
                images: vec![ImageInput {
                    mime: "audio/wav".into(),
                    label: String::new(),
                    bytes: vec![]
                }]
            })
            .is_err()
        );
    }
    #[test]
    fn terminal_reason_is_not_assumed_success() {
        assert_eq!(
            parse_line(r#"{"done":true,"done_reason":"length"}"#).unwrap(),
            vec![GenerationEvent::Finished { complete: false }]
        );
        assert!(parse_line(r#"{"error":"broken"}"#).is_err());
        assert!(parse_line("not json").is_err());
    }
    #[tokio::test]
    async fn fragmented_unicode_survives_line_decoder() {
        let bytes = "{\"message\":{\"content\":\"café 🦀\"}}\n"
            .as_bytes()
            .to_vec();
        let parts = futures_util::stream::iter(
            bytes
                .into_iter()
                .map(|b| Ok::<_, std::io::Error>(axum::body::Bytes::from(vec![b]))),
        );
        let mut lines = FramedRead::new(
            StreamReader::new(parts),
            LinesCodec::new_with_max_length(65536),
        );
        assert_eq!(
            parse_line(&lines.next().await.unwrap().unwrap()).unwrap(),
            vec![GenerationEvent::Delta("café 🦀".into())]
        );
    }
}

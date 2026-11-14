//! Groq text/vision generation. Extraction, speech and embeddings stay independent.
use crate::providers::{Capabilities, ChatTurn, GenerationEvent, Generator, TokenStream};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt};
use serde_json::{Value, json};
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    io::StreamReader,
};

pub struct Groq {
    client: reqwest::Client,
    model: String,
    key: Option<String>,
    vision: bool,
    max_images: usize,
}
impl Groq {
    pub fn configured(client: reqwest::Client) -> Result<Self> {
        Self::for_role(client, false)
    }
    pub fn for_role(client: reqwest::Client, vision: bool) -> Result<Self> {
        let key = if let Ok(path) = std::env::var("GROQ_API_KEY_FILE") {
            Some(
                std::fs::read_to_string(path)
                    .context("Cannot read GROQ_API_KEY_FILE")?
                    .trim()
                    .to_owned(),
            )
        } else {
            std::env::var("GROQ_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty())
        };
        Ok(Self {
            client,
            key,
            vision,
            max_images: std::env::var("VISION_MAX_IMAGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3usize)
                .clamp(1, 3),
            model: std::env::var(if vision { "VISION_MODEL" } else { "CHAT_MODEL" })
                .unwrap_or_else(|_| {
                    if vision {
                        "qwen/qwen3.8-27b".into()
                    } else {
                        "openai/gpt-oss-20b".into()
                    }
                }),
        })
    }
    pub fn key(&self) -> Result<&str> {
        self.key
            .as_deref()
            .filter(|k| !k.is_empty())
            .context("Groq needs GROQ_API_KEY or GROQ_API_KEY_FILE. Run python3 scripts/setup.py")
    }
}
fn message(turn: ChatTurn, vision: bool) -> Result<Value> {
    if !vision && !turn.images.is_empty() {
        bail!(
            "This Groq adapter is text-only; route images through the independent vision generator"
        );
    }
    if turn.images.is_empty() {
        return Ok(json!({"role":turn.role,"content":turn.content}));
    }
    use base64::Engine;
    let mut content = vec![json!({"type":"text","text":turn.content})];
    for image in turn.images {
        content.push(json!({"type":"text","text":image.label}));
        content.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",image.mime,base64::engine::general_purpose::STANDARD.encode(image.bytes))}}));
    }
    Ok(json!({"role":turn.role,"content":content}))
}
fn parse_data(data: &str) -> Result<Vec<GenerationEvent>> {
    // [DONE] alone does not prove the provider completed an answer.
    if data == "[DONE]" {
        bail!("Groq closed without a finish reason");
    }
    let value: Value = serde_json::from_str(data).context("Malformed Groq stream event")?;
    if value.get("error").is_some() {
        bail!("Groq reported a streaming error");
    }
    let Some(choice) = value["choices"].as_array().and_then(|v| v.first()) else {
        return Ok(vec![]);
    };
    let mut events = vec![];
    if let Some(text) = choice["delta"]["content"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        events.push(GenerationEvent::Delta(text.into()));
    }
    if let Some(reason) = choice["finish_reason"].as_str() {
        events.push(GenerationEvent::Finished {
            complete: reason == "stop",
        });
    }
    Ok(events)
}
#[async_trait]
impl Generator for Groq {
    fn model(&self) -> &str {
        &self.model
    }
    fn max_images(&self) -> usize {
        if self.vision { self.max_images } else { 0 }
    }
    fn provider(&self) -> &str {
        "groq"
    }
    async fn capabilities(&self) -> Result<Capabilities> {
        let response = self
            .client
            .get("https://api.groq.com/openai/v1/models")
            .bearer_auth(self.key()?)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(
                "Groq model access check returned HTTP {}",
                response.status()
            );
        }
        let value: Value = response.json().await?;
        if !value["data"]
            .as_array()
            .is_some_and(|items| items.iter().any(|m| m["id"] == self.model))
        {
            bail!("Configured Groq model is not available to this account");
        }
        Ok(Capabilities {
            text: true,
            image_input: self.vision,
        })
    }
    async fn generate(
        &self,
        system: String,
        prompt: ChatTurn,
        history: Vec<ChatTurn>,
    ) -> Result<TokenStream> {
        if self.vision && self.model != "qwen/qwen3.8-27b" {
            bail!("This vision adapter currently validates qwen/qwen3.8-27b only");
        }
        if prompt.images.len() + history.iter().map(|t| t.images.len()).sum::<usize>()
            > self.max_images()
        {
            bail!(
                "The selected provider accepts at most {} images per request",
                self.max_images()
            );
        }
        let mut messages = vec![json!({"role":"system","content":system})];
        for turn in history.into_iter().chain(std::iter::once(prompt)) {
            messages.push(message(turn, self.vision)?);
        }
        let mut body = json!({"model":self.model,"messages":messages,"stream":true,"temperature":0.1,"max_completion_tokens":2048});
        if self.model.starts_with("openai/gpt-oss-") {
            body["reasoning_effort"] = json!("low");
        }
        let response = self
            .client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(self.key()?)
            .json(&body)
            .send()
            .await?;
        // Never place a provider body or credential in public errors or logs.
        if !response.status().is_success() {
            let message = match response.status().as_u16() {
                429 => {
                    "The model provider's rate limit was reached. Wait about a minute before retrying; ask your instructor if the limit persists."
                }
                413 => {
                    "This input exceeds the model provider's account allowance. Use fewer images or shorter text, or ask your instructor about provider limits."
                }
                _ => {
                    "The model provider could not start. Check credentials, model access and quota."
                }
            };
            return Err(crate::providers::ProviderFailure(message.into()).into());
        }
        let reader = StreamReader::new(response.bytes_stream().map_err(std::io::Error::other));
        let mut lines = FramedRead::new(reader, LinesCodec::new_with_max_length(65536));
        Ok(Box::pin(async_stream::try_stream! {
            let mut data=String::new();
            while let Some(line)=lines.next().await {
                let line=line?;
                if line.is_empty() {
                    if data.is_empty() { continue; }
                    for event in parse_data(&data)? {
                        let done=matches!(event,GenerationEvent::Finished{..});
                        yield event;
                        if done { return; }
                    }
                    data.clear();
                } else if let Some(part)=line.strip_prefix("data:") {
                    if !data.is_empty(){data.push('\n');}
                    data.push_str(part.trim_start());
                    if data.len()>65536 { Err(anyhow::anyhow!("Groq stream frame exceeds limit"))?; }
                }
            }
            Err(anyhow::anyhow!("Groq stream ended without a terminal event"))?;
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_finish_and_text_only_contract() {
        assert_eq!(
            parse_data(r#"{"choices":[{"delta":{"content":"café 🦀"},"finish_reason":null}]}"#)
                .unwrap(),
            vec![GenerationEvent::Delta("café 🦀".into())]
        );
        assert_eq!(
            parse_data(r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#).unwrap(),
            vec![GenerationEvent::Finished { complete: false }]
        );
        assert_eq!(
            parse_data(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#).unwrap(),
            vec![GenerationEvent::Finished { complete: true }]
        );
        assert!(parse_data("[DONE]").is_err());
        assert!(parse_data(r#"{"error":{"message":"hidden"}}"#).is_err());
        assert!(
            message(
                ChatTurn {
                    role: "user".into(),
                    content: "hi".into(),
                    images: vec![crate::providers::ImageInput {
                        mime: "image/jpeg".into(),
                        label: "picture".into(),
                        bytes: vec![1]
                    }]
                },
                false
            )
            .is_err()
        );
    }
}

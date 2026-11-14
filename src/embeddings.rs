//! Small local CPU model service. Identity includes the actual downloaded artifact hash.
use crate::providers::{Embedder, EmbeddingIdentity};
use anyhow::{Result, bail};
use async_trait::async_trait;
use serde_json::{Value, json};
pub struct FastEmbed {
    client: reqwest::Client,
    base: String,
}
impl FastEmbed {
    pub fn configured(client: reqwest::Client) -> Result<Self> {
        let base =
            std::env::var("EMBEDDING_URL").unwrap_or_else(|_| "http://embeddings:8080".into());
        let url = reqwest::Url::parse(&base)?;
        if url.scheme() != "http"
            || !matches!(
                url.host_str(),
                Some("embeddings" | "127.0.0.1" | "localhost")
            )
            || !url.username().is_empty()
            || url.password().is_some()
        {
            bail!("Embedding service must be a local HTTP endpoint");
        }
        Ok(Self {
            client,
            base: base.trim_end_matches('/').into(),
        })
    }
}
#[async_trait]
impl Embedder for FastEmbed {
    fn is_local(&self) -> bool {
        true
    }
    async fn identity(&self) -> Result<EmbeddingIdentity> {
        Ok(self
            .client
            .get(format!("{}/identity", self.base))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    async fn embed(&self, texts: Vec<String>, query: bool) -> Result<Vec<Vec<f32>>> {
        let expected = texts.len();
        let result: Value = self
            .client
            .post(format!("{}/embed", self.base))
            .json(&json!({"texts":texts,"query":query}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let vectors: Vec<Vec<f32>> = serde_json::from_value(result["embeddings"].clone())?;
        if vectors.len() != expected
            || vectors
                .iter()
                .any(|v| v.len() != 384 || v.iter().any(|x| !x.is_finite()))
        {
            bail!("Invalid local embeddings");
        }
        Ok(vectors)
    }
}

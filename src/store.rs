use crate::providers::{Embedder, EmbeddingIdentity};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Once};

#[derive(Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub file: String,
    pub origin: String,
    pub permission: String,
    pub version: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub paragraph: usize,
    pub text: String,
    pub origin: String,
    pub score: f64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub identity: EmbeddingIdentity,
    pub dimensions: usize,
    pub corpus_hash: String,
    pub source_count: usize,
    pub chunk_count: usize,
    pub indexed_at: u64,
}

pub fn open(path: &str) -> Result<Connection> {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        // sqlite-vec's documented SQLite extension registration ABI.
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut std::ffi::c_char,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> std::ffi::c_int,
        >(
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    });
    let db = Connection::open(path)?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS metadata (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS sources (id TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS chunks (rowid INTEGER PRIMARY KEY, value TEXT NOT NULL);")?;
    Ok(db)
}
pub fn metadata(db: &Connection) -> Result<IndexMeta> {
    let raw: String = db
        .query_row("SELECT value FROM metadata WHERE id=1", [], |r| r.get(0))
        .context("No index. Run cargo run -- import")?;
    Ok(serde_json::from_str(&raw)?)
}
pub fn sources(db: &Connection) -> Result<Vec<Source>> {
    let mut stmt = db.prepare("SELECT value FROM sources ORDER BY id")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}
pub fn chunk_text(source: &Source, text: &str) -> Result<Vec<Chunk>> {
    let normalized = text.replace("\r\n", "\n");
    let mut chunks = Vec::new();
    for (i, paragraph) in normalized.split("\n\n").enumerate() {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() || paragraph.starts_with("# ") {
            continue;
        }
        if paragraph.len() > 4000 {
            bail!(
                "{} paragraph {} exceeds 4000 bytes; split it before import",
                source.id,
                i + 1
            );
        }
        chunks.push(Chunk {
            id: format!("{}:p{}", source.id, i + 1),
            source_id: source.id.clone(),
            title: source.title.clone(),
            paragraph: i + 1,
            text: paragraph.into(),
            origin: source.origin.clone(),
            score: 0.0,
        });
    }
    Ok(chunks)
}

pub async fn import(
    path: &str,
    corpus: &Path,
    embedder: &dyn Embedder,
) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(corpus.join("manifest.json"))?;
    let sources: Vec<Source> = serde_json::from_str(&raw)?;
    if sources.is_empty() || sources.len() > 100 {
        bail!("Corpus must have 1–100 sources");
    }
    let mut hash = Sha256::new();
    hash.update(raw.as_bytes());
    let mut chunks = Vec::new();
    let mut ids = std::collections::HashSet::new();
    let base = corpus.canonicalize()?;
    for source in &sources {
        if source.id.is_empty()
            || !source
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("Source IDs must contain only letters, digits, hyphens or underscores");
        }
        let origin = reqwest::Url::parse(&source.origin).context("Source origin must be a URL")?;
        if !matches!(origin.scheme(), "http" | "https") {
            bail!("Source origin must use http or https");
        }
        if source.permission.trim().is_empty() || source.version.trim().is_empty() {
            bail!("Each source needs permission and version metadata");
        }
        if !ids.insert(source.id.clone()) {
            bail!("Duplicate source ID");
        }
        let file = corpus.join(&source.file).canonicalize()?;
        if !file.starts_with(&base) {
            bail!("Source must be inside corpus directory");
        }
        if std::fs::metadata(&file)?.len() > 100_000 {
            bail!("Source exceeds 100 KB");
        }
        let text = std::fs::read_to_string(file)?;
        hash.update(source.id.as_bytes());
        hash.update(text.as_bytes());
        chunks.extend(chunk_text(source, &text)?);
    }
    if chunks.is_empty() || chunks.len() > 1000 {
        bail!("Corpus must produce 1–1000 chunks");
    }
    let corpus_hash = format!("{:x}", hash.finalize());
    let identity = embedder.identity().await?;
    let mut db = open(path)?;
    if let Ok(old) = metadata(&db)
        && old.identity == identity
        && old.corpus_hash == corpus_hash
    {
        return Ok(serde_json::json!({"status":"unchanged","index":old}));
    }
    let mut vectors = Vec::new();
    for batch in chunks.chunks(16) {
        vectors.extend(
            embedder
                .embed(
                    batch
                        .iter()
                        .map(|c| format!("{}\n{}", c.title, c.text))
                        .collect(),
                    false,
                )
                .await?,
        );
    }
    let dimensions = vectors.first().context("No embeddings")?.len();
    if vectors.len() != chunks.len()
        || dimensions == 0
        || vectors.iter().any(|v| v.len() != dimensions)
    {
        bail!("Inconsistent embedding dimensions");
    }
    // Model tags can move. Never publish vectors created across a model replacement.
    if embedder.identity().await? != identity {
        bail!("Embedding model changed during import; retry");
    }
    let meta = IndexMeta {
        identity,
        dimensions,
        corpus_hash,
        source_count: sources.len(),
        chunk_count: chunks.len(),
        indexed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };
    // Build all embeddings before the transaction. Replacement is atomic on failure.
    let tx = db.transaction()?;
    tx.execute_batch(&format!("DROP TABLE IF EXISTS vectors; DELETE FROM sources; DELETE FROM chunks;
        CREATE VIRTUAL TABLE vectors USING vec0(embedding float[{dimensions}] distance_metric=cosine);"))?;
    for source in &sources {
        tx.execute(
            "INSERT INTO sources VALUES (?1,?2)",
            params![source.id, serde_json::to_string(source)?],
        )?;
    }
    for (i, (chunk, vector)) in chunks.iter().zip(vectors).enumerate() {
        tx.execute(
            "INSERT INTO chunks(rowid,value) VALUES (?1,?2)",
            params![i as i64, serde_json::to_string(chunk)?],
        )?;
        tx.execute(
            "INSERT INTO vectors(rowid,embedding) VALUES (?1,?2)",
            params![i as i64, serde_json::to_string(&vector)?],
        )?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO metadata VALUES (1,?1)",
        [serde_json::to_string(&meta)?],
    )?;
    tx.commit()?;
    Ok(serde_json::json!({"status":"indexed","index":meta}))
}

pub fn vector_search(db: &Connection, vector: &[f32], expected: &IndexMeta) -> Result<Vec<Chunk>> {
    let current = metadata(db)?;
    if current.identity != expected.identity
        || current.corpus_hash != expected.corpus_hash
        || vector.len() != current.dimensions
    {
        bail!("Index changed or dimensions differ; retry after reindexing");
    }
    let mut stmt = db.prepare(
        "SELECT c.value,v.distance FROM vectors v JOIN chunks c ON c.rowid=v.rowid
        WHERE v.embedding MATCH ?1 AND k=4 ORDER BY v.distance",
    )?;
    let rows = stmt.query_map([serde_json::to_string(vector)?], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    rows.map(|r| {
        let (raw, distance) = r?;
        let mut chunk: Chunk = serde_json::from_str(&raw)?;
        chunk.score = 1.0 - distance;
        Ok(chunk)
    })
    .collect()
}
pub fn keyword_search(db: &Connection, question: &str) -> Result<Vec<Chunk>> {
    let terms: Vec<String> = question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 2)
        .map(str::to_lowercase)
        .collect();
    let mut stmt = db.prepare("SELECT value FROM chunks")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut chunks: Vec<Chunk> = rows
        .map(|r| -> Result<Chunk> { Ok(serde_json::from_str(&r?)?) })
        .collect::<Result<_>>()?;
    for chunk in &mut chunks {
        let text = chunk.text.to_lowercase();
        chunk.score = terms.iter().filter(|t| text.contains(t.as_str())).count() as f64;
    }
    chunks.retain(|c| c.score > 0.0);
    chunks.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
    chunks.truncate(4);
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestEmbedder {
        fail: bool,
        revision: &'static str,
    }
    #[async_trait::async_trait]
    impl Embedder for TestEmbedder {
        async fn identity(&self) -> Result<EmbeddingIdentity> {
            Ok(EmbeddingIdentity {
                provider: "test-only".into(),
                model: "fixture".into(),
                revision: self.revision.into(),
            })
        }
        async fn embed(&self, texts: Vec<String>, _query: bool) -> Result<Vec<Vec<f32>>> {
            if self.fail {
                bail!("Injected provider failure");
            }
            Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
        }
    }
    #[tokio::test]
    async fn imports_are_idempotent_atomic_and_revision_aware() {
        let root = std::env::temp_dir().join(format!(
            "is373-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = Source {
            id: "s".into(),
            title: "Test".into(),
            file: "s.txt".into(),
            origin: "https://example.test/source".into(),
            permission: "test".into(),
            version: "1".into(),
        };
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_string(&vec![source]).unwrap(),
        )
        .unwrap();
        std::fs::write(root.join("s.txt"), "# Title\n\nFirst paragraph.").unwrap();
        let path = root.join("test.sqlite").to_string_lossy().into_owned();
        let good = TestEmbedder {
            fail: false,
            revision: "1",
        };
        assert_eq!(
            import(&path, &root, &good).await.unwrap()["status"],
            "indexed"
        );
        assert_eq!(
            import(&path, &root, &good).await.unwrap()["status"],
            "unchanged"
        );
        let before = metadata(&open(&path).unwrap()).unwrap();
        std::fs::write(
            root.join("s.txt"),
            "# Title\n\nChanged paragraph.\n\nAdditional paragraph.",
        )
        .unwrap();
        assert!(
            import(
                &path,
                &root,
                &TestEmbedder {
                    fail: true,
                    revision: "1"
                }
            )
            .await
            .is_err()
        );
        assert_eq!(
            metadata(&open(&path).unwrap()).unwrap().corpus_hash,
            before.corpus_hash
        );
        import(&path, &root, &good).await.unwrap();
        assert_eq!(metadata(&open(&path).unwrap()).unwrap().chunk_count, 2);
        assert!(vector_search(&open(&path).unwrap(), &[1.0, 0.0], &before).is_err());
        import(
            &path,
            &root,
            &TestEmbedder {
                fail: false,
                revision: "2",
            },
        )
        .await
        .unwrap();
        assert_eq!(
            metadata(&open(&path).unwrap()).unwrap().identity.revision,
            "2"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn paragraphs_preserve_locations_and_unicode() {
        let source = Source {
            id: "s".into(),
            title: "Title".into(),
            file: "s.txt".into(),
            origin: "https://example.test/source".into(),
            permission: "test".into(),
            version: "1".into(),
        };
        let chunks = chunk_text(&source, "# Title\r\n\r\nCafé 🦀\r\n\r\nSecond.").unwrap();
        assert_eq!(chunks[0].id, "s:p2");
        assert_eq!(chunks[0].text, "Café 🦀");
        assert_eq!(chunks[1].paragraph, 3);
    }
    #[test]
    fn real_sqlite_vector_query_ranks_nearest() {
        let db = open(":memory:").unwrap();
        db.execute_batch(
            "CREATE VIRTUAL TABLE test_vec USING vec0(embedding float[2] distance_metric=cosine);
            INSERT INTO test_vec VALUES (1,'[1,0]'),(2,'[0,1]');",
        )
        .unwrap();
        let nearest: i64 = db
            .query_row(
                "SELECT rowid FROM test_vec WHERE embedding MATCH '[0.9,0.1]' AND k=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(nearest, 1);
    }
}

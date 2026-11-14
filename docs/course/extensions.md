# Extend the system without coupling every layer

Start with a bounded behavior and an observable acceptance criterion. Keep the existing working path as a baseline. Use [architecture](architecture.md) to identify the module that owns the change rather than adding vendor logic to the UI or file parsing to a model adapter.

## Add an online generator

`Generator` accepts normalized messages and yields application generation events. Read [providers.rs](../../src/providers.rs) and [groq.rs](../../src/groq.rs). A new OpenAI, Anthropic or DeepSeek adapter must map the current official API, supported models, content types, finish reasons, refusals, limits and stream framing into that contract. Similar JSON shapes do not guarantee identical semantics. Check current official vendor documentation when implementing it.

Keep endpoints validated, credentials server-side, response frames/timeouts bounded and public errors free of secrets. Declare text/image capability and locality accurately. Register the adapter in `configured()`, then test a supported answer, an unavailable model, an interrupted stream, malformed data, Unicode split across bytes and a token limit. Reuse the same corpus and manually review source support. Do not silently retry a partially delivered answer with another provider.

A hosted text adapter does not provide embeddings or transcription by implication. Vision can implement `Generator` separately; speech implements `Transcriber`; embeddings implement `Embedder`. Embedding changes require an atomic index rebuild. PDF extraction and video decoding can stay local while their normalized output goes to a hosted model, so update data-flow documentation and UI capability reports.

## Promote an upload into the library

Today attachments belong to conversation context. A general ingestion feature needs an explicit user choice to index a permitted file, source/version metadata, stable page/timestamp positions, deduplication, chunking, embeddings and a transaction that activates the complete revision. Do not put every uploaded file into a shared corpus automatically.

Start with text PDFs only and test source attribution end to end. Add scanned-page OCR or captions as separately labeled derived evidence, preserving the original and model provenance. Decide how a change invalidates old chunks and whether historical answers still resolve their old citations. A durable queue needs persisted job states, leases, cancellation, retry policy, idempotency and recovery after process restart; an in-memory task alone does not provide that.

## Build toward public operation

Implement identity and per-user ownership before opening network access. Every conversation, original file and generated asset route needs authorization, including range/download requests. Add per-user quotas and retention/deletion semantics. Separate long media processing from request workers and define durable run/event APIs if clients must reconnect and replay.

Then test HTTPS/reverse-proxy streaming, request limits, session/CSRF policy, model egress, backup/restore and deployment rollback. The current single-request semaphore is deliberately simple; replacing it requires measuring memory pressure and queue behavior under load. Add observability with redacted structured logs and identifiers, not full prompts or credentials by default.

Possible small exercises: add a retrieval-only evaluation view; show exact retained context boundaries; implement a permitted text-PDF import command; compare chunk overlap using the same question set; add one provider with contract tests. Each should have a clear input, expected outcome and measured limitation.

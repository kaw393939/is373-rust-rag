# Provider-independent boundaries

`src/providers.rs` defines inference traits; `src/processors.rs` defines independent document/video extraction traits. Text and vision use separate instances of `Generator`:

| Interface | Contract | Current adapter |
| --- | --- | --- |
| `Embedder` | Identify provider/model/revision; batch document or query embeddings | Ollama `/api/embed`, Nomic task prefixes |
| `Generator` (text or vision) | Accept system + typed user/assistant history containing text and validated image bytes; yield text deltas and explicit completion | Ollama `/api/chat` with bounded NDJSON; Groq text SSE in `src/groq.rs` |
| `Transcriber` | Accept audio bytes/MIME; return text, language, duration and model provenance | whisper.cpp with FFmpeg normalization |
| `DocumentExtractor` | PDF bytes → numbered text pages and optional page images | Poppler |
| `VideoDecoder` | Video bytes → timestamped images, optional PCM audio and coverage warnings | FFmpeg/ffprobe |

`src/main.rs` depends only on these traits and translates the generation stream to stable browser SSE events: `run.started`, `input.progress`, `input.ready`, `retrieval.evidence_selected`, `answer.delta`, `run.ended`. Outcomes are complete, incomplete, refused, failed or interrupted. Browser cancellation is local and shown as cancelled. Disconnect/EOF is not successful completion.

## Adding OpenAI, Anthropic or DeepSeek later

Implement a new `Generator` adapter in this module or a child module, then register it in `configured()`. The adapter owns credentials, endpoint validation, vendor request/response formats, Unicode-safe stream decoding, error normalization and finish reasons. Provider errors stay in server logs; public errors are generic. The UI must never receive credentials or parse vendor-specific streams. API keys belong to server environment configuration. Groq text/vision generation, Groq speech and a local FastEmbed service adapter are implemented; OpenAI, Anthropic and DeepSeek remain future adapters. Groq accepts `GROQ_API_KEY` or `GROQ_API_KEY_FILE`. The frontend never receives these values. See [model setup](course/models.md) for data destinations and configuration.

Generation and embeddings need not use the same vendor. An Anthropic or DeepSeek generator can use an independent embedding service, including the existing local embedder. Do not infer embedding support from chat compatibility. Confirm each vendor's current API and supported models when implementing its adapter.

Changing the generator does not invalidate vectors. A new embedding provider/model/revision **does** invalidate the index even if dimensions happen to match. Re-embed all sources and atomically replace the vector index. Pin model revisions where the provider offers them; document aliases that can change. The current Ollama adapter records and compares the installed model digest.

## Adapter acceptance checks

Reuse the same retrieval/evaluation corpus across providers. Test fragmented Unicode, malformed frames, upstream errors, missing terminal events, length-limit outcomes, bounded timeouts and cancellation. Verify cited IDs against retrieved chunks and manually check support. Do not silently retry generation after partial output or fall back to another provider: that can duplicate cost and change data destinations.

This student-sized stream is connection-owned and ephemeral. It intentionally does not claim the leased workers, durable run IDs, event replay or authorization model proposed in the larger `is373-rag` research architecture.

API references consulted: [Ollama embeddings](https://docs.ollama.com/api/embed), [Ollama chat](https://docs.ollama.com/api/chat), [Axum SSE](https://docs.rs/axum/latest/axum/response/sse/index.html), [sqlite-vec Rust integration](https://alexgarcia.xyz/sqlite-vec/rust.html).

## Conversation workspace update

The reusable shell stores attachment metadata independently of provider messages. Text attachments, PDF pages and audio transcripts become reference context. Validated images, scanned PDF pages and sampled video frames reach the vision generator with explicit page/frame labels. Video audio goes through the same independent `Transcriber` used for voice notes. Neither the frontend nor generator adapters need to decode PDF/video containers. The `/api/status` capability record is checked against installed models. `input.progress` announces preprocessing and the selected model; `input.ready` delivers saved attachment metadata, page text, frame asset IDs and transcripts. Research evidence uses stable page/frame/audio IDs, with sources pointing to the original PDF page or generated image asset. Transcription caching is keyed by model digest, language and preprocessing version. New adapters must map these normalized input types, validate capabilities and retain explicit failures. See [chat-platform.md](chat-platform.md) for the durable conversation and attachment API.

Vision can move to a different provider from text generation; speech can stay local or move separately. The frontend sends attachment IDs and consumes application SSE, never vendor payloads. A text-only provider is not assumed to offer vision, embeddings or transcription. See [multimedia.md](multimedia.md) for routing and cache details.

Local PDF extraction and frame decoding can remain unchanged when text, vision or speech moves online. The application assembles normalized text and labeled `ImageInput` values, and checks required vision support before generation. Future adapters should declare locality and capabilities accurately; changing the provider must not imply that media stays on this machine. The current frontend derives its privacy labels from these reports.

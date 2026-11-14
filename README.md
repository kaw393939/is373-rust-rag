# IS373 Rust RAG · Ordo conversation workspace

[![Test and publish](https://github.com/kaw393939/is373-rust-rag/actions/workflows/ci.yml/badge.svg)](https://github.com/kaw393939/is373-rust-rag/actions/workflows/ci.yml)

**Start with the [complete student guide](docs/course/README.md).** It covers system requirements, installation, technology foundations, ETL, architecture, model setup, provider credentials, Docker, CI/CD, guided labs, evaluation and troubleshooting.

[GitHub repository](https://github.com/kaw393939/is373-rust-rag) · [Docker Hub image](https://hub.docker.com/r/kaw393939/is373-rust-rag) · [Instructor workshop guide](docs/course/teaching.md)

A polished Svelte conversation interface and Rust/Axum backend: persistent chat history, rich streamed responses, multimedia messages, and optional source-grounded RAG. Open **http://127.0.0.1:3730** after starting the server.

Text generation, vision, speech transcription and embeddings use independent provider interfaces. Ollama, whisper.cpp and optional Groq text generation are implemented; OpenAI, Anthropic and DeepSeek adapters can be added later. The frontend consumes a stable application API rather than vendor-specific streams.

## Guided setup

Install the prerequisites for your chosen path from [requirements and installation](docs/course/setup.md), then run:

```sh
python3 scripts/setup.py
python3 scripts/start.py
```

The assistant chooses local or Groq text generation and native or Docker execution. It installs model files, imports the corpus and stores configuration privately in ignored `.env.local`. The Groq path uses a hidden key prompt. Local embeddings and optional local vision/speech remain independent. Docker includes the app and media executables; model weights are downloaded into persistent volumes.

## Manual native setup

Prerequisites: Rust/Cargo (tested 1.94), Node.js 22.22+, and [Ollama](https://ollama.com/). For multimedia, install FFmpeg, whisper.cpp and Poppler (`brew install ffmpeg whisper-cpp poppler` on macOS; official whisper.cpp build instructions are linked below). Gemma 3, Nomic embeddings and Whisper small occupy roughly 4.2 GB on disk. Downloads need internet; the default local configuration keeps inference and storage on your machine afterward. Selecting Groq sends text context to that service; see [models and data flow](docs/course/models.md).

```sh
# Start only if Ollama is not already running:
ollama serve
```

In another terminal, from the repository root:

```sh
ollama pull nomic-embed-text
bash scripts/setup_media.sh
npm ci --prefix frontend
npm --prefix frontend run build
cargo run --locked -- import
cargo run --locked -- serve
```

Open http://127.0.0.1:3730. Stop with Ctrl-C. For frontend development, keep Rust running and use `npm --prefix frontend run dev`; Vite proxies API calls to port 3730.

## The experience

- **Conversations:** create, search titles/recent prompts, rename, pin, archive, restore and export Markdown. Reloading a conversation URL restores its saved messages.
- **Rich answers:** streamed Markdown, tables, code highlighting, copy buttons, explicit stop/error/limit states and saved partial responses.
- **Branching:** edit a message or retry a response in a new branch, preserving the original exchange.
- **Multimedia:** attach/paste/drop images, audio, video, PDF and text; image previews, native playback and downloads. Record an audio note with microphone permission.
- **Two modes:** Chat for general assistance; Research for answers grounded in the collection. Open source cards in a separate evidence drawer. Settings retains a keyword comparator.
- **Personal workspace:** responsive navigation, light/dark/system themes and keyboard shortcuts.

**Model inputs:** images are understood by local Gemma 3; audio files and microphone notes are transcribed by local whisper.cpp, with an expandable saved transcript. Follow-ups retain images/transcripts within the context budget. Research mode can cite these attachments. PDFs supply page text and rendered scans. Videos supply timestamped frame samples and speech transcripts. See [multimedia setup and behavior](docs/multimedia.md).

## ETL today

There are two ingestion paths, both local by default:

| Path | Extract | Transform | Load |
| --- | --- | --- | --- |
| Knowledge library (`cargo run -- import`) | Read the corpus and source manifest | Split paragraphs; create embeddings; detect content/model changes | Atomically update source records and SQLite vectors |
| Chat attachments (on send) | Decode files; extract PDF pages, sampled video frames and speech | Normalize images, transcribe audio, create evidence IDs and processing metadata | Save originals, derived images and text in SQLite; supply bounded model context |

Attachment processing is conversation-scoped. It does **not** automatically add uploads to the shared vector index. The current pipeline has no durable job queue, scheduled synchronization, watched folders or general PDF/video library import command. Provider adapters are independent of extraction and storage. See [multimedia](docs/multimedia.md) and [provider boundaries](docs/providers.md).

## Source-grounded teaching example

The five synthetic Harbor Club documents come from the user's [IS373 RAG reference lab](https://github.com/kaw393939/is373-rag/tree/main/student-reference). The source manifest records IDs, origin, permission and version. Paragraph chunks retain source positions; real Nomic embeddings are stored in SQLite with sqlite-vec for exact cosine search. A keyword baseline supports comparison.

`cargo run -- import` hashes the corpus and embedding model digest. Unchanged input is a no-op. Changed sources/models rebuild the small vector index atomically after embeddings succeed. `data/rag.sqlite` stores vectors, conversations and attachment blobs and is ignored by Git. The source library remains independent of message attachments.

Try “Who organizes workshops?” or “When is next year's annual meeting?” Research mode shows retrieved evidence and validates citation IDs. That validation does not prove factual support; the small local model has produced misattributed claims in testing. The [original evaluation record](docs/verification.md) preserves that finding.

## Reuse and configuration

- [Chat platform architecture and API](docs/chat-platform.md) — components, multimedia contract, persistence and operational boundaries.
- [Provider design](docs/providers.md) — independent generation, vision, transcription and embedding adapters.
- [Multimedia](docs/multimedia.md) — supported inputs, limits, configuration and real inference tests.
- [Course repository review](docs/repository-review.md) — why this example follows the student assignment's source-to-answer path.
- [Containers and CI/CD](docs/course/delivery.md) — tested AMD64/ARM64 images, registry credentials, releases and rollback.
- [.env.example](.env.example) — environment reference; the Python setup/start scripts manage `.env.local`, while direct Cargo commands do not load it automatically.

Changing `CHAT_MODEL` requires restart, not reindexing. Changing the embedding model/provider/revision requires import again. A new generation adapter must also implement its supported content types; a model setting alone does not enable multimodal analysis.

## Verify

```sh
cargo test --locked
npm --prefix frontend test
npm --prefix frontend run build
# With the app and Ollama running:
python3 scripts/check_chat.py
# With the media fixtures described in docs/multimedia.md:
python3 scripts/check_multimedia.py
# With the document/video fixtures described in docs/multimedia.md:
python3 scripts/check_documents_video.py
python3 scripts/evaluate.py
```

`check_chat.py` exercises conversation/attachment APIs, branching, local-model follow-ups, archive/restore and origin isolation. It archives its test conversations. `evaluate.py` records real retrieval and model answers in `data/evaluation.json`; assess source support manually as well as automatic markers.

## Deployment boundary

This is a working **single-user local app**, not a publicly deployed multi-user service. It binds loopback and rejects untrusted browser Origins/Host headers. It has no account authentication, per-user authorization, event replay or attachment garbage collection. CI tests and publishes containers; it does not deploy a public production host. Further operational work is required before a public production launch; see the [platform notes](docs/chat-platform.md).

Limits: one inference at a time, four attachments/message, eight visual inputs in active context (images, scan pages and video frames), PDFs up to 20 pages / 16,000 extracted characters, 20 megapixels/image (resized to 1536 px), two minutes/audio or video, 12 MB/media file, 24 KB/text file, 500 MB total attachments, 12,000 characters/question, bounded history, 2,048 output tokens and 300-second generation/compound-processing timeouts. Archives retain data. The UI clearly reports incomplete/interrupted generation. No model answer is treated as an authorized action.

The existing npm audit reports low-severity build-tool advisories rooted in deepmerge without an available automated fix. Build tooling is not served as executable application code. Dependency lockfiles are included.

The [official IS373 individual brief](https://github.com/kaw393939/is373-studio/blob/main/docs/projects/individual.md) and Canvas remain authoritative. This instructor example adds no required student stack and is not a completed hosted student submission.

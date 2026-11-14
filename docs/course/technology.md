# Technology foundations

Read this alongside the running app. Each technology solves a specific problem; none replaces understanding the full path from source to answer.

## The browser and the server

The browser renders the interface and sends HTTP requests. It does not hold provider credentials or call model vendors directly. The Rust server validates requests, reads stored conversations, processes attachments, retrieves evidence and calls the selected model adapter. Both use JSON for structured data. Serde converts Rust structures to and from JSON; it does not automatically validate whether a claim or uploaded document is trustworthy.

HTTP methods describe operations. `GET` reads a resource, `POST` creates data or starts work, and `PATCH` changes selected fields. A successful transport status does not prove a model answer is correct. A chat request returns a stream whose final application event describes its outcome. See the API table in [the platform reference](../chat-platform.md).

## Rust, Cargo, Axum and Tokio

Rust checks ownership and borrowing to control how values are shared and released. `Arc` shares ownership of provider instances across asynchronous requests. `Result<T, E>` makes failure explicit. Traits define behavior without requiring callers to know a particular implementation. Here, `Generator` describes an answering model and `Embedder` describes a vector-producing model; switching one does not require switching the other.

Cargo resolves dependencies, builds the executable and runs tests. `Cargo.lock` records exact dependency versions. The repository's toolchain file pins the compiler used in CI. Axum provides HTTP routing, JSON extraction, multipart upload handling and SSE responses. Tokio schedules asynchronous tasks and manages network I/O and child processes. `async` allows a task to yield while waiting; it does not make CPU-heavy decoding free. The application limits concurrent inference with a semaphore.

Dropping an in-flight stream releases its resources. Child processes use kill-on-drop and timeouts. Persistent run records preserve partial answers and record interruption. This is cancellation of the current request, not durable job orchestration or restartable background work.

Primary reading: [Rust Book](https://doc.rust-lang.org/book/), [Tokio tutorial](https://tokio.rs/tokio/tutorial), [Axum documentation](https://docs.rs/axum/latest/axum/).

## Svelte, Vite and safe rich text

Svelte components represent the composer, messages, attachment cards and dialogs. Application state decides what is visible: uploading, extracting, transcribing, generating, complete or failed. Vite runs the development server and produces the static production bundle. Rust serves that bundle, giving the browser one origin for the UI and API.

A model's Markdown is untrusted input. `marked` parses it, selected Highlight.js languages format code, and DOMPurify removes unsafe markup. Executable links and remote image embeds are restricted. A response containing HTML must not become a script running in the student's browser. The sanitization tests exercise this trust boundary.

An accessible interface also needs labels, keyboard behavior, readable error states and predictable scrolling. The quality of the UI and the quality of the model are separate concerns. A polished answer card can contain an incorrect answer.

## Models, tokens and embeddings

A generation model predicts output tokens given a context. Tokens are pieces of text, not necessarily words or characters. The context window includes instructions, evidence, history and the new message. This application uses character and image budgets as a small local approximation; those limits are not exact tokenizer accounting.

An embedding model maps text to a numeric vector. Similar wording or meaning may produce nearby vectors, but an embedding is not a stored answer. Nomic embeddings here have 768 dimensions. The adapter applies different task prefixes for document text and questions. A generator then receives the retrieved text and writes an answer.

RAG supplies external evidence at request time. It does not fine-tune the generation model or change its weights. Saved conversation history is also not model training. Distinguish model weights, the vector index, attachment assets and conversation records when discussing what the system remembers.

## SQLite and sqlite-vec

SQLite stores data in a local database file. This project uses it for source metadata, chunks, vectors, conversations and attachment blobs. Transactions make a group of changes atomic: either the replacement index is committed or the earlier index remains. WAL mode supports the application's read/write workload; it is not a backup strategy.

sqlite-vec adds vector operations to SQLite. The application performs exact top-four cosine-neighbor retrieval over a small collection. Cosine similarity compares vector direction; it is useful for semantic candidates, not proof of factual relevance. The keyword comparator counts matching terms and provides a simple baseline.

Ten passages do not require a distributed vector service. A larger workload may need a different index, database, authorization model and operational plan. Choosing SQLite here is a bounded design decision, not a claim that it is optimal for every system.

## Streaming and UTF-8

Server-Sent Events (SSE) carry named text events over an HTTP response. The client incrementally decodes bytes because a network packet can split a multi-byte character or a JSON event. Seeing a closed connection is not evidence that generation completed. The client requires `run.ended`, and the adapters require an explicit upstream finish signal.

The application events are independent of vendors. Ollama uses newline-delimited JSON upstream; Groq uses SSE. Both adapters normalize their responses to text deltas and completion events before the HTTP layer streams them to the browser.

## Local media tools

Poppler inspects PDFs, extracts their text and renders scan pages. FFmpeg/ffprobe inspect media, decode sampled video frames and normalize audio. whisper.cpp runs a Whisper speech-recognition model and returns a transcript. Ollama runs the local text/vision/embedding models and exposes an HTTP API.

These stages have different error modes. A correct transcript can still be misused by a generator; a mistaken transcript can be repeated perfectly. A sampled video frame cannot establish what happened between samples. Inspect the intermediate data before changing the prompt.

## Containers, registries and CI/CD

A Docker image is a packaged filesystem plus runtime configuration. A container is a running instance. A volume persists data outside the container's writable layer. A registry stores images; tags are convenient names, while digests identify exact image content. An ARM64 image and an AMD64 image contain different machine code; a multi-platform manifest lets Docker select the correct one.

Continuous integration checks each change. Continuous delivery publishes a tested artifact. In this repository, CI runs code checks and container smoke tests, then publishes an image from trusted branches/tags. It does not automatically expose a public server or provision production credentials. See [delivery](delivery.md).

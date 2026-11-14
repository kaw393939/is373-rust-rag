# Ordo conversation workspace

This is the reusable conversation layer on the Rust RAG server. It runs locally at http://127.0.0.1:3730. The UI is designed as a complete conversation product rather than a retrieval dashboard. The default local profile remains available; the separate production profile adds invitation accounts and private account storage. See [authentication](course/authentication.md), [conversational administration](course/control-plane.md) and [deployment](course/production.md).

## Working user flows

- New conversations, saved history, title/recent-prompt search, rename, pin, archive and restore.
- Multi-turn chat with bounded server-loaded context; active conversation restored from its URL on reload.
- Edit a user message or retry an assistant response by creating a separate branch. Original history is retained.
- Streamed Markdown, highlighted code, code/response copying, tables, lists and blockquotes. Unsafe HTML, executable links and remote image embeds are stripped.
- Chat mode for general assistance; Research mode for source-grounded answers. A source drawer preserves exact retrieved passages. The keyword comparator remains available in Settings → Research retrieval.
- A composer with Enter to send, Shift+Enter for a newline, paste/drop/file-picker attachments, upload state, microphone recording, explicit stop, and scroll-to-latest without intentionally pulling readers away from older messages.
- Light, dark and system appearance, responsive sidebar, keyboard search (Cmd/Ctrl+K), new chat (Cmd/Ctrl+Shift+O), native modal focus/escape behavior, reduced-motion support.
- Conversation export to Markdown. Image lightbox, native audio/video playback and downloads.

## Multimedia contract

Attachments are server-stored UUID records with display names, signature-checked MIME, size and optional input state, transcript, extracted document or sampled video metadata. Messages refer to these records. Uploads are bounded; image bytes are decoded and resized before inference. SVG/HTML are rejected. Downloads retain `nosniff`, sandbox headers, safe filenames and byte-range playback.

Images are passed to the separate vision generator. Audio is normalized by FFmpeg and transcribed by the `Transcriber` adapter; saved transcripts become model context and can be inspected/exported. Follow-ups preserve image/transcript context within budgets. Research can cite attachment sources. PDFs supply page text or rendered scanned pages; videos supply sampled frames and optional audio transcripts. Saved page/frame details are inspectable, image previews open generated assets, and Markdown exports include extracted content and sampling coverage. See [multimedia.md](multimedia.md) for exact formats, processing limits, cache keys and operational behavior.

## Reusable frontend pieces

| File | Responsibility |
| --- | --- |
| `frontend/src/components/Message.svelte` | User/assistant messages, actions, sources, notices |
| `frontend/src/components/RichText.svelte` | Sanitized Markdown and highlighted/copyable code |
| `frontend/src/components/Attachment.svelte` | Compact composer tiles and full message media |
| `frontend/src/components/Modal.svelte` | Native dialog lifecycle and focus boundary |
| `frontend/src/lib/api.js` | HTTP error normalization, attachment URLs, Markdown export |
| `frontend/src/lib/markdown.js` | Markdown parsing, strict HTML allowlist and code rendering |
| `frontend/src/stream.js` | Incremental UTF-8/SSE lifecycle validation |
| `frontend/src/App.svelte` | Workspace/conversation orchestration and view state |
| `frontend/src/style.css` | Shared design tokens, light/dark themes and responsive layouts |

The shell uses same-origin `/api` calls. To reuse it with another Rust deployment, preserve these contracts or adapt the API module and stream endpoint. Branding and suggested prompts live in the shell; visual tokens live on `.app` in the stylesheet. Model/provider responses do not reach UI components directly.

## Rust API

| Method / path | Behavior |
| --- | --- |
| `GET /api/conversations` | Lightweight summaries, newest first, including archived records |
| `POST /api/conversations` | Create a conversation |
| `GET /api/conversations/{id}` | Load messages, attachment metadata and retrieved evidence |
| `PATCH /api/conversations/{id}` | Update title, pinned or archived state |
| `POST /api/conversations/{id}/fork` | `{ "before": "message-id" }`; copy preceding history to a new conversation |
| `POST /api/attachments` | Multipart file upload; returns validated attachment metadata |
| `GET /api/attachments/{id}` | Download or play stored content, with byte-range support |
| `GET /api/attachments/{id}/assets/{asset_id}` | Generated JPEG belonging to this attachment; private immutable caching |
| `POST /api/chat` | `{ "conversation_id": "…", "question": "…", "mode": "chat|vector|keyword", "attachments": ["…"] }`; SSE response |
| `GET /api/health` | Lightweight process health, independent of model readiness |
| `GET /api/status` | Provider/model, embedding index and capabilities |
| `GET /api/sources` | Indexed source manifest |
| `POST /api/search` | Retrieval-only comparison API |

`conversation_id` is optional for compatibility with the original evaluation script. Without it, the response is ephemeral. Stored runs append user + assistant records before inference; stream events carry their IDs. The server saves bounded partial batches approximately every 250 ms when tokens arrive, plus terminal state. On disconnect, a drop guard saves partial text and marks the run interrupted. On server restart, stale streaming records become interrupted. A hard crash can lose text since the last committed batch. There is no automatic retry or SSE replay.

Generation, embeddings, transcription, document extraction and video decoding have independent traits. Generation now accepts typed user/assistant history in addition to system instructions and the current prompt. The Ollama adapter owns vendor JSON and stream decoding. The Groq adapter maps text/vision SSE into the same contract and its speech adapter returns normalized transcripts; other hosted adapters remain unimplemented.

## Operational scope

In `AUTH_MODE=local`, the native service binds loopback. The container binds internally on all interfaces while Compose publishes only to host loopback. Requests with non-loopback Host headers or untrusted browser Origins are rejected; the Vite dev origin on port 5173 is allowed. UI responses disallow framing, use no-referrer and nosniff. These controls do not replace authentication. Trusted local processes can call the API.

One inference request runs at a time. Generation is capped at 2,048 tokens and provider calls at 300 seconds. Input questions are capped at 12,000 characters, attached/current context at 24,000 characters, and historical context at up to 12 complete messages within a character budget. Character budgets are a small local approximation, not exact tokenizer accounting. Ollama uses a 16,384-token context. Long discussions can fall outside retained model context while remaining in saved history.

The production profile implements invitation identity, session/Origin policy, account-scoped data, quotas, hosted media adapters, backups and deployment gates. Still pending: paginated history, durable jobs/event replay, user-controlled deletion/retention, billing, MFA/SSO and high-availability operations. The default loopback profile remains intentionally simple.

## Verification

`cargo test --locked` covers vector/import contracts, stream parsing, missing references, file-type rejection, range handling and disconnect persistence. `npm --prefix frontend test` covers stream decoding and rich-text sanitization. `python3 scripts/check_chat.py` exercises live conversation persistence, branching, uploads/ranges, attached-text generation, follow-up context, archive/restore and origin isolation. It archives its generated test conversations.

Browser QA additionally covers desktop/narrow layouts, streaming rich text, stop, image previews, native media controls, themes, source inspection and restored conversations. Specific results are recorded in the implementation's verification notes. A local model can still produce incorrect content; UI rendering and reference resolution are not factuality checks.

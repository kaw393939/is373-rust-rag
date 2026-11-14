# Verification · November 13–14, 2026

This is a chronological engineering record. Earlier sections describe earlier capability boundaries; the latest delivery section records the current implementation. Known model-quality failures remain preserved.

## Executed checks

- `cargo test --locked`: six passing tests. Missing/unknown citation detection; real sqlite-vec nearest-neighbor ranking; stable paragraph locations; fragmented Unicode decoding; error/finish-reason parsing; and an import lifecycle covering no-op reimports, failed-provider rollback, changed-content replacement and changed-model revision. The import test uses explicitly test-only embeddings; the live app uses actual Ollama vectors.
- `npm --prefix frontend test`: three passing stream-consumer tests for one-byte Unicode fragments, premature EOF and invalid event order.
- `npm --prefix frontend run build`: production Svelte/Vite bundle generated successfully.
- Live import: five sources, ten passages, 768 dimensions. Repeated unchanged import returned `unchanged` with the same index timestamp and corpus fingerprint.
- Live HTTP: UI returns 200; empty/oversized questions and unknown retrieval modes return 400. Concurrent inference returns 429. Closing an active stream releases capacity and the next search succeeds.
- Missing chat model: returns an explicit `run.ended` failure after retrieval; never reports complete.
- Browser: source library contains all five documents. Submitting the membership question displays retrieved paragraphs before completion, then displays `The annual membership fee is $15 [membership:p2].` with Complete status. The narrow browser viewport uses the responsive stacked layout.
- An additional unrelated question about Neptune produced an insufficient-evidence answer, not an answer from general model knowledge.

## Live model evaluation

The initial run exposed a missing citation on the paraphrase answer. A more explicit citation instruction with a formatting example corrected the first observed failure, but a later run omitted a citation again. The server now also marks uncited or unknown-reference answers incomplete, while accepting a canonical insufficient-evidence response. This validates reference IDs, not claim support. The original relationship expectation was also corrected: the yes/no question asks whether the secretary approves applications, not who approves them. Abstentions with no factual claim do not require a citation. Initial and current raw results are retained in ignored `data/evaluation-initial.json` and `data/evaluation.json` on this machine.

The five cases check an exact fee, a paraphrase about learning sessions, a secretary/application relationship, visitor sign-in and an unannounced annual-meeting date. The generated answers were inspected against the actual retrieved text. Machine checks cover expected evidence, answer markers, citation-ID resolution and terminal completion; they are not an automatic factuality judge. The first cold run took about 65 seconds for its first question; subsequent warm runs took roughly 2.4–7.5 seconds per question. Timing varies by hardware and model state.

The paraphrase demonstrates the chosen vector approach: vector search retrieves `events:p2`, while the simple keyword top-four omits it. The relationship answer correctly says the secretary does not approve applications; the vector top-four does not include the separate passage naming the Membership Committee, so it appropriately stops at what was asked.

## Model and storage configuration

- Generation: Ollama `llama3.2:latest`, installed digest `a80c4f17acd55265feec403c7aef86be0c25983ab279d83f3bcd3abbcb5b8b72`, 3.2B Q4_K_M.
- Embeddings: Ollama `nomic-embed-text:latest`, installed digest `0a109f422b47e3a30ba2b10eca18548e944e8a23073ee3f3e947efcf3c45e59f`, 768 dimensions, query/document task prefixes.
- sqlite-vec 0.1.6, exact cosine top-four search; bundled SQLite through rusqlite. Dependency versions are locked in Cargo.lock and frontend/package-lock.json.

## Limits of this verification

No cloud provider, public deployment, proxy/HTTPS streaming, multi-user authorization, durable replay or graph database was tested or implemented. One successful model evaluation does not guarantee future citation correctness or abstention. Desktop-wide layout was not separately browser-tested; the live browser was narrow. `npm audit` reports three low-severity development dependency advisories rooted in deepmerge, with no available automated fix. These remain documented rather than hidden.

Final run after the citation guard: all five live cases passed every automated check. The checked-in compact record is [evaluation-results.json](evaluation-results.json); full event traces remain in the local data directory.

### Manual finding in the final run

**Four of five final answers pass manual source-support review.** The relationship answer cites `membership:p3` for the secretary not approving applications, but that paragraph only documents submission and receipt confirmation. `committee:p3` supports non-approval. The answer additionally says the application process does not include approval, an unsupported generalization. This remains a known local-model quality issue, not a passing factuality check. Earlier runs answered this correctly, demonstrating run-to-run variability even with temperature zero. The reference-ID guard catches absent/unknown citations but cannot prove entailment. Before calling this a checked release, improve grounding or the local model and retest support, not just IDs. The compact JSON records this manual failure explicitly.

## Conversation workspace upgrade

The later Ordo UI/backend work is described in [chat-platform.md](chat-platform.md). The earlier five-question RAG record above remains historical; it is not a claim that all later model outputs are factual.

Executed on November 13, 2026:

- Eight Rust tests pass, including new media type/range and interrupted-message persistence checks.
- Five frontend tests pass, including executable HTML/link/image sanitization and highlighted-code escaping.
- Production Svelte build succeeds. The browser bundle is approximately 86 KB gzipped, with only selected syntax languages included.
- `scripts/check_chat.py` passes live origin/host isolation, upload/type rejection, byte ranges, real attached-text answers, follow-up context, stored message equality, branching without altering the source, rename/pin/archive/restore and archived-write rejection.
- Browser upload through the actual file chooser succeeds for JPEG, WAV, MP4 and UTF-8 text together. All four appear as message attachments. Native audio and video playback both advanced their current time with readyState 4 and no media errors.
- The model correctly extracted the three design priorities from the uploaded text note. The image/audio/video were explicitly preview-only.
- Browser stop preserves a partial code response. Reloading the multimedia conversation restores its messages and attachments from its URL. Renaming updates the title and sidebar. Light and dark appearance were visually checked at the desktop breakpoint; the narrow layout was also inspected.

Limits: microphone permission/recording was implemented but not exercised against the user's microphone; audio-file upload/playback was tested. There is no public deployment or multi-user authorization test. The model's generated code was used to inspect rendering and stop behavior, not certified as correct code.

Final browser checks also confirmed Research mode returns the supported $15 answer with its source drawer, reload preserves the conversation/research context, and the 390-pixel layout has no horizontal document overflow. The final desktop screenshot is saved locally at data/qa/ordo-desktop.jpg. The media upload route accepted a 96,044-byte WAV above the JSON request limit, confirming its separate upload limit.

## Local multimodal inputs upgrade

The app now passes actual decoded image bytes to a separate vision generator and actual audio through whisper.cpp. Text/vision default to `gemma3:4b` (installed digest prefix `68ceab7e468a`), with independently configurable providers/models. Speech uses multilingual Whisper base, SHA-256 `60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe`. Nomic and the original index remain unchanged.

Twelve Rust tests and seven frontend tests pass. New coverage includes vendor image encoding, image decode/size normalization, silence/duration rejection, transcript caching by configuration, historical transcript snapshots, media stream lifecycle and capability labels. `cargo clippy --all-targets --locked -- -D warnings` also passes. These unit tests complement real inference; their fake transcriber is confined to the cache contract test.

The initial live audio test exposed a model-quality problem with Llama 3.2: recognition was correct, but a follow-up contradicted itself and the research answer missed the clearly transcribed workshop date. Gemma 3 answered those facts correctly, but an initial research run emitted a literal citation placeholder. The reference guard marked it incomplete. The grounding prompt was changed to list actual allowed citations explicitly and require copying IDs verbatim. Initial local traces remain in `data/multimedia-initial.json` and `data/multimedia-gemma-initial.json`.

Audio fixtures are synthetic speech generated locally with macOS `say`. The screenshot fixture is the actual Ordo UI. This does not test the user's physical microphone, arbitrary accents/noisy recordings, speaker analysis or realtime voice conversation. PDF/video understanding and cloud adapters are not implemented.

Manual visual review: Gemma correctly identified “Ordo” and both large headline lines from the screenshot. In its initial brand-name answer it also invented “Ordo Labs 3.2” as a version, apparently misreading the visible `llama3.2` model label. The targeted fact checks do not certify extra claims in a response. This is a remaining local-model accuracy limit; the UI and provider integration do not eliminate hallucinations.

Final multimedia integration run: all eight scenarios pass the targeted assertions in `scripts/check_multimedia.py`: WAV transcription/answering, audio follow-up, cached transcript with an exact research citation, screenshot brand recognition, image follow-up headline reading, exact image research citation, corrupt audio rejection and both Opus/WebM + AAC/M4A transcription. Completed responses match their persisted records. QA conversations were archived. The compact actual results are preserved in [multimedia-results.json](multimedia-results.json).

The production frontend build passes (approximately 89 KB gzipped JavaScript). The repeatable media setup script passed against the installed models and verified the Whisper model checksum. Local inference latency varied considerably during testing, especially image requests and cold runs; progress events and cancellation remain available during preprocessing and inference.

The five original library evaluations were rerun with Gemma 3 and the new grounding prompt: all automated retrieval, marker, citation-ID and completion checks pass. Manual source review passes four answers completely. The secretary answer first uses the correct `committee:p3` citation, then repeats that claim beside `membership:p3`, which only supports application submission/receipt. This remains a citation-support limitation despite correct overall content. See [evaluation-gemma-results.json](evaluation-gemma-results.json); the earlier Llama evaluation is preserved separately.

Browser verification used the actual file chooser to upload the screenshot and WAV together. The UI showed preparation, a saved expandable transcript, image-reading status and streamed output. The answer identified Ordo and Friday at 3 PM from the separate inputs, while also adding the same unreliable product-version interpretation noted above. Reload restored the conversation, both attachments, the transcript and the saved answer. The demonstration remains open at `/#chat=8f0bcd16-5858-449f-a60b-39524d019ef4`. A screenshot of the restored transcript is saved at `data/qa/ordo-model-media.jpg`.

## PDF and video model inputs

PDFs now use local Poppler page extraction, with rendered images for pages without a useful text layer. Videos use local FFmpeg/ffprobe frame sampling and the same independent speech adapter as audio attachments. Extracted page text, generated JPEGs, transcript provenance and sampling warnings are saved with conversation snapshots. The underlying interfaces remain independent of the answer provider.

Verification for this change:

- `cargo clippy --all-targets --locked -- -D warnings`, `cargo build --locked` and 15 Rust tests pass. One native-decoder test is normally ignored because it requires local binaries and generated fixtures; it was explicitly run and passed.
- That native test processes real text/scanned PDFs, narrated and silent MP4, and a WebM without a duration header. It checks page boundaries, generated image bytes, frame timestamps, the blue middle frame's pixels and audio presence.
- Eight frontend tests pass; the production build succeeds at approximately 90 KB gzipped JavaScript. Tests cover stream lifecycle, sanitization and truthful processed-media labels.
- The synthetic PDF pages and middle video frame were rendered and visually inspected. The encrypted, corrupt, 21-page and over-16,000-character PDF fixtures all fail explicitly through the live API, with no invented answer.
- Initial live testing passed PDF page answers/citations, follow-up page context, three video frames, exact frame citations, frame-asset parent association, persisted snapshots and scanned-page recognition. Whisper base misheard the video's access word “cedar” as “seater”; the answer model faithfully repeated that recognition error. The full integration script continued remaining contract checks and exited nonzero for speech accuracy. The unchanged initial results are preserved in [document-video-base-results.json](document-video-base-results.json).

The same unchanged MP4 audio was then tested with multilingual Whisper small. It recognized “Cedar” correctly, including the Tuesday 10 AM schedule. Whisper small is now the default (`ggml-small.bin`, SHA-256 `1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b`); the smaller base model remains configurable. Model downloads are checksum-verified. Old conversation transcripts remain unchanged. This is an observed improvement on this fixture, not a general transcription-accuracy guarantee.

Coverage limits remain explicit: three default frame samples do not cover all motion; text-layer PDF extraction can omit figures/table layout; rendered scans rely on model vision rather than saved OCR; there is no general video event timeline, speaker identification or background-sound analysis. No cloud adapter, public hosting or multi-user authorization was added.

## Student delivery and Groq adapter · November 14

The final PDF/video integration run passes all nine targeted scenarios with Whisper small, including page citations/follow-ups, scanned-page recognition, video frame citations and speech, and explicit encrypted/corrupt/over-budget PDF failures. See [document-video-results.json](document-video-results.json). Browser QA uploaded the real PDF and MP4 through the file chooser, inspected extracted pages and the blue middle-frame preview, and confirmed the saved transcript and answer after reload. The answer correctly identified the orange notebook, blue middle frame and Tuesday 10 AM schedule.

A real Groq key was configured privately for testing; it is not part of this repository or the image. Groq `openai/gpt-oss-20b` streamed the supported $15 membership answer and the orange notebook answer from a locally extracted PDF. Both completed responses matched their saved records and contained the expected citation IDs. Initial output used Unicode citation brackets and was marked incomplete; explicit ASCII citation instructions corrected the observed contract failure. Decorative outer Unicode brackets can still appear around a valid ASCII citation. This is a narrow two-case adapter check, not a broad quality comparison. Compact results are in [groq-results.json](groq-results.json).

The Docker image builds locally on ARM64. Its runtime smoke check passes non-root UID, HTTP health, mapped-port Origin handling, native FFmpeg/Poppler/Whisper executables, real PDF extraction, three sampled video frames and stored JPEG retrieval. This check deliberately runs without model weights or a cloud key and expects inference to report unavailable. It tests packaging and extraction rather than substituting for the real model checks above.

The educational guide documents installation paths, practical hardware estimates, model roles/downloads, credentials, two ETL paths, request/event lifecycles, CI/CD, labs, evaluation and deployment boundaries. The setup configuration is tested for non-execution and owner-only file permissions. No production host or public multi-user service has been deployed.

Final local checks: 16 Rust tests pass (plus the previously executed native-decoder test), 8 frontend tests pass, formatting and Clippy pass, the UI production build succeeds, 2 setup/configuration tests pass, all relative Markdown file links resolve, and Compose configuration validates. A separate real container test imported five sources/ten Nomic passages through host Ollama and streamed a supported $15 membership answer using Gemma.

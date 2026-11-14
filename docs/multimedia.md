# Local documents, images, video and speech

The app uses three independently selected inference roles: text chat (`gemma3:4b`), image conversations (`gemma3:4b`) and speech recognition (whisper.cpp with the multilingual Whisper small model). The roles are independently configurable even though text and vision default to the same model, reducing local memory pressure. Nomic embeddings remain separate. Poppler extracts PDF pages; FFmpeg/ffprobe decode video independently of the inference providers. Install the binaries and run `bash scripts/setup_media.sh`, then restart the server. No cloud account or API key is needed.

## Conversation behavior

Attach an image and ask about its contents. The Rust server validates and decodes it, fits it within 1536 × 1536 pixels, encodes it as JPEG and passes actual image bytes through the `Generator` contract. Ollama alone translates these to its base64 `images` field. Images above 20 megapixels are rejected; GIF input uses its first frame. Original files remain available for preview/download.

Attach an audio file or record a voice note, then send. Processing events appear immediately while FFmpeg normalizes audio to mono 16 kHz PCM and whisper.cpp performs local recognition. The resulting transcript is saved on the attachment, displayed under **View transcript**, included in Markdown exports and supplied to the answering model. Supported containers: WAV, MP3, OGG, FLAC, M4A and audio WebM. Audio-only WebM is recognized by a `.weba` filename or `audio/webm` upload type. The maximum is two minutes and 12 MB per file. Malformed, silent, overlong and empty-recognition inputs fail explicitly rather than yielding an invented success.

Whisper is not perfect dictation. Transcripts can mishear names and numbers or hallucinate on noise. The silence check rejects very low energy input; it is not a general speech detector. The assistant sees text, not original sound, and cannot evaluate speaker identity, emotion or background audio. There is no realtime voice conversation or speech output in this implementation.

Attach a PDF to extract text locally with Poppler while preserving page boundaries. Pages with fewer than eight alphanumeric text characters are rendered to JPEG and sent to the vision model. This supports scanned PDFs without pretending that OCR text was extracted. Expand **View PDF content** to inspect the exact extracted text or rendered pages. Text-layer pages are not also rendered, so figures, diagrams and complex table layout on those pages may be absent from model context. For a diagram question, attach the relevant page as an image. Encrypted, corrupt and over-budget PDFs fail explicitly. Limits: 20 pages, 16,000 extracted characters and four rendered pages per PDF, with no silent truncation.

Attach MP4 or WebM video to sample frames and transcribe the first audio track. The default is three still images at uniform interval midpoints, scaled to fit 768 × 768 pixels. A nine-second clip supplies approximately 1.5, 4.5 and 7.5 second views. Frame timestamps describe sampling targets; seeks are approximate. Clips shorter than half a second use one frame. Browser WebM files without duration headers use a bounded packet-timestamp scan. Limits are two minutes, 12 MB and a 20-megapixel decoded frame size. Expand **View sampled frames** to inspect exactly what the model received, and **View transcript** for the speech text. No audio track and no recognized speech are distinct visible states. Decoder or transcription failures are explicit and do not silently drop audio. This is sampled-frame understanding, not continuous motion analysis; short events between frames can be missed.

Recent conversation history includes saved PDF text, rendered pages, video frames, images and transcripts, so follow-up questions can use them after reload. Up to eight visual inputs total are included across the current message and bounded history. Current input exceeding that budget fails with a clear message; older turns are omitted once image/text budgets are reached. Previously saved PDF/video messages from before this feature have no processed snapshot and must be reattached.

Research mode includes attachment evidence with stable IDs: `[attachment-UUID:pN]` for PDF pages (or `p1` for standalone images/audio), `[attachment-UUID:fN]` for video frames and `[attachment-UUID:audio]` for video speech. These supplement the indexed library for this conversation; uploads are not automatically added to the vector index. Source cards for images describe the attached input, not an independently verified caption. Citation validation checks reference IDs, not correctness of visual interpretation or speech recognition.

## Architecture and configuration

`src/providers.rs` owns `Generator`, `Embedder`, `Transcriber`, normalized `ChatTurn`/`ImageInput`, `Transcript` and capability types. `src/speech.rs` implements the whisper.cpp adapter. `src/processors.rs` owns `DocumentExtractor` and `VideoDecoder` plus local Poppler/FFmpeg adapters. `src/media.rs` resolves stored attachment IDs, caches derived assets and prepares evidence. Image parts carry labels identifying their PDF page or video timestamp; each generator adapter owns its vendor-specific encoding. No browser component parses an Ollama or Whisper payload.

`VISION_PROVIDER`/`VISION_MODEL` configure the image-capable generator independently of `CHAT_PROVIDER`/`CHAT_MODEL`. Any included image, including one in recent history, selects the vision generator for the entire turn. `TRANSCRIPTION_PROVIDER` selects speech recognition independently. `/api/status` checks installed Ollama model capabilities, speech binary/model presence and document/video binaries. PDF text can work with a text-only generator; scanned pages and video frames require vision. Presence checks cannot guarantee that an individual file will decode or that an inference process will succeed; run failures remain explicit.

The setup script defaults to Whisper small. For a smaller download, run `WHISPER_VARIANT=base bash scripts/setup_media.sh` and export `WHISPER_MODEL=data/models/ggml-base.bin` before starting Rust. The base model misrecognized a word in our video fixture, so small is now the default; neither provides guaranteed accuracy. Existing historical transcripts are preserved.

Speech configuration: `WHISPER_MODEL`, `WHISPER_CLI`, `WHISPER_LANGUAGE` (default `auto`) and `FFMPEG_BIN`. Adapters also report whether processing is local; privacy labels derive from these values rather than a hardcoded vendor name. The model's SHA-256 plus requested language and preprocessing version form the transcript cache key. Reattaching with a changed configuration regenerates the transcript; historical messages retain the transcript originally used. Restart after configuration or model changes. No embedding reindex is required for vision or speech changes.

Document/video configuration: `PDF_PROVIDER=poppler`, `VIDEO_PROVIDER=ffmpeg`, `PDFINFO_BIN`, `PDFTOTEXT_BIN`, `PDFTOPPM_BIN`, `FFMPEG_BIN`, `FFPROBE_BIN` and `VIDEO_SAMPLE_FRAMES` (default 3, allowed 1–4). The environment file is documentation; export overrides before starting the server. Pipeline versions, binary paths and sampling settings form processing cache keys; video adds the speech cache key. Binary upgrades at an unchanged path are not automatically detected, so re-upload to force fresh extraction after upgrading tools. Historical messages retain their original metadata and derived images even when later processing changes configuration.

Extraction, video decoding and transcription use bounded child processes with argument arrays, no shell expansion, restricted FFmpeg protocols, timeouts and kill-on-drop cancellation. Temporary PDF, image, video, audio and JSON files are removed on completion, error or cancellation. Transcripts, extracted page text, sampled images and original uploads remain in the app's local SQLite storage. Original and generated image blobs share the 500 MB quota. Derived JPEG assets are served only under their parent attachment ID; there is still no multi-user authorization. Compound preprocessing has a 300-second timeout, with tighter limits on individual subprocesses. These processes are not an OS sandbox; keep native decoders updated. Downloaded models are ignored by Git.

New cloud adapters should map the same normalized inputs to their own APIs and report genuine model capabilities. A text-only online generator can still use an independent vision provider and local or online transcription. No OpenAI, Anthropic or DeepSeek cloud adapter is implemented yet; capability support must be checked when adding one. See [providers.md](providers.md).

## Document/video verification

Generate local fixtures with `python3 scripts/make_document_video_fixtures.py` (requires ReportLab, Pillow, pypdf, FFmpeg and macOS `say`). This creates text/scanned/encrypted/over-budget PDFs in ignored `tmp/pdfs` and narrated/silent MP4 plus durationless WebM in ignored `data/qa/compound`. The speech fixture is synthetic; no user's microphone is recorded.

Run `cargo test --locked real_document_and_video_decoding -- --ignored` to exercise actual Poppler/FFmpeg, including page extraction, scanned rendering, the middle frame's pixel color, audio presence and durationless WebM. Run `python3 scripts/check_documents_video.py` against the live server for real model answers, page/frame citations, persisted follow-ups, asset-parent checks and explicit PDF failures. It records `data/document-video-results.json` and archives QA conversations. It checks speech accuracy separately from successful plumbing and exits nonzero if a recognized word is wrong. See [verification.md](verification.md) for the observed Whisper base error and the small-model comparison; successful processing does not certify recognition accuracy.

## Image/audio inference verification

`python3 scripts/check_multimedia.py` expects two ignored QA fixtures: `data/qa/interface.jpg`, a screenshot of the Ordo interface, and `data/qa/spoken-note.wav`. It tests real local vision and speech, follow-up context, persisted transcripts, research citations and corrupt-audio failure. Outputs are retained in `data/multimedia-results.json`. It archives its QA conversations. Model output assertions can fail; inspect the saved answer before drawing a conclusion.

On macOS, create the speech fixture locally:

```sh
mkdir -p data/qa
say -v Samantha -o data/qa/spoken-note.aiff \
  'The workshop starts on Friday at three in the afternoon. Please bring a blue notebook. The access code is seven four nine.'
ffmpeg -y -i data/qa/spoken-note.aiff -ar 16000 -ac 1 data/qa/spoken-note.wav
# Optional compressed-format checks (also accepted from browser voice recording):
ffmpeg -y -i data/qa/spoken-note.wav -c:a libopus -f webm data/qa/spoken-note.weba
ffmpeg -y -i data/qa/spoken-note.wav -c:a aac data/qa/spoken-note.m4a
```

References: [Ollama vision inputs](https://docs.ollama.com/capabilities/vision), [Gemma 3 model](https://ollama.com/library/gemma3:4b), [whisper.cpp](https://github.com/ggml-org/whisper.cpp), [official model download script](https://github.com/ggml-org/whisper.cpp/blob/master/models/download-ggml-model.sh).

Decoder references: [Poppler pdftotext manual](https://manpages.debian.org/bookworm/poppler-utils/pdftotext.1.en.html), [FFmpeg probing](https://ffmpeg.org/ffprobe.html), [FFmpeg filters](https://ffmpeg.org/ffmpeg-filters.html).

The default speech model download is checksum-pinned to the [whisper.cpp small model](https://huggingface.co/ggerganov/whisper.cpp/blob/main/ggml-small.bin), SHA-256 `1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b`.

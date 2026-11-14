# Models, providers and credentials

A provider is the service or runtime used to run a model. A model is the trained set of weights identified by a name/version. A model role is the job assigned by the application. These are different choices.

| Role | Current default | Configuration | Input/output |
| --- | --- | --- | --- |
| Text generation | Ollama `gemma3:4b` | `CHAT_PROVIDER`, `CHAT_MODEL` | Conversation text → streamed answer |
| Optional hosted text | Groq `openai/gpt-oss-20b` | `CHAT_PROVIDER=groq`, `CHAT_MODEL` | Text/evidence → streamed answer |
| Vision generation | Ollama `gemma3:4b` | `VISION_PROVIDER`, `VISION_MODEL` | Labeled images plus text → answer |
| Embeddings | Ollama `nomic-embed-text:latest` | `EMBEDDING_PROVIDER`, `EMBEDDING_MODEL` | Text → numeric vectors |
| Speech recognition | whisper.cpp + multilingual Whisper small | `WHISPER_MODEL`, `WHISPER_LANGUAGE`, `WHISPER_CLI` | Normalized audio → transcript |
| PDF extraction | Poppler | `PDF_PROVIDER`, tool paths | PDF → page text/rendered scan pages |
| Video decoding | FFmpeg/ffprobe | `VIDEO_PROVIDER`, `VIDEO_SAMPLE_FRAMES` | Video → sampled images and audio |

Text and vision happen to use the same local model by default to reduce memory pressure. They remain separate adapter instances and can be configured independently. A model being good at text does not imply it accepts images, produces embeddings or transcribes audio.

## Local installation and model files

Ollama stores its own downloaded model files. The setup script pulls Nomic and, when needed, Gemma. `scripts/download_whisper.sh` downloads Whisper small separately, verifies a pinned SHA-256 and saves it under `data/models` (or the configured `WHISPER_MODEL` path). Docker puts Whisper and the app database in `app-data`; the optional Ollama container uses its own `ollama-data` volume.

Weights are deliberately absent from the Git repository and application image. This keeps code releases separate from large model downloads and their licensing/version choices. Downloading a model does not mean it is loaded in memory. A first request can be slower while weights and context are prepared.

For a smaller speech model, use `WHISPER_VARIANT=base` with the download script and set `WHISPER_MODEL` accordingly. The base model misheard a synthetic video access word in testing; small recognized it correctly on the same audio. This is a measured example, not a guarantee for arbitrary speech. Keep transcripts inspectable.

## Groq setup and data flow

Select Groq through `scripts/setup.py`, or set the server environment explicitly. The adapter sends requests only to `https://api.groq.com/openai/v1`. It implements text streaming and checks that the configured model is available to the account. It does not implement Groq vision, embeddings or speech APIs.

The name `openai/gpt-oss-20b` identifies an open-weight model served by Groq. This path uses a **Groq** key and Groq's service; it is not an OpenAI Platform integration. Check the [current Groq model list](https://console.groq.com/docs/models) before changing models. Availability, account limits and prices can change; the application does not assume unlimited free usage.

Text-only turns may send questions, bounded history, retrieved passages, PDF text and audio transcripts to Groq. Turns containing actual images, scan pages or sampled video frames select the independently configured vision generator, which remains local here. A later hosted vision adapter would change that data destination. The UI's privacy labels derive from adapter locality rather than a hardcoded local-only promise.

## Keys are runtime configuration

The server accepts `GROQ_API_KEY` or `GROQ_API_KEY_FILE`. The file variant is useful for a production secret mounted at runtime. Never bake a key into a Dockerfile, build argument, frontend bundle, screenshot, source manifest or committed example. GitHub Actions only needs the registry publishing credential to build/push the image; it does not need an inference key for deterministic CI.

The setup assistant uses a hidden prompt and a private ignored configuration file. The instructor's local credential is not distributed with this repository. Students obtain their own key through the [Groq console](https://console.groq.com/keys), use appropriate project permissions and record expiration/rotation requirements outside public code. Avoid sharing one instructor key across student laptops.

For production, install a credential file on the target host with appropriate ownership and restrictive permissions, mount it read-only and set `GROQ_API_KEY_FILE` to the container path. A bind-mounted file must be readable by the image's non-root UID 10001. Do not solve a permission error by making the key world-readable. A secret manager or deployment platform can handle this more cleanly than a manually copied file.

Creating a Docker image or saving a key locally does not deploy that key to a remote host. Decide the target host, identity, access policy, backup/rotation procedure and network exposure before a production rollout. The current application remains a single-user reference.

## Changing models safely

Change one role at a time and keep a baseline result. A generation-model change needs a restart and fresh answer evaluation, but does not invalidate embeddings. An embedding model/provider/revision change needs a re-import even if dimensions match. A speech-model/language change changes the transcript cache key; historical messages retain the original transcript. Reattach/reprocess to obtain a new transcript under the current configuration.

The application does not silently fall back from local to cloud or retry partial generation with another provider. Such a fallback could change cost, privacy and the meaning of a saved answer. Surface the failure and make the provider decision explicit.

Primary references: [Ollama API](https://docs.ollama.com/api), [Groq compatibility](https://console.groq.com/docs/openai), [Groq GPT-OSS 20B](https://console.groq.com/docs/model/openai/gpt-oss-20b), [whisper.cpp](https://github.com/ggml-org/whisper.cpp).

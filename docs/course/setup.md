# Requirements and installation

Run commands from the repository root unless a step says otherwise. Start with a small text conversation before adding every media format. The automated setup script is an assistant for installing model files and building/importing the application; it does not silently install operating-system packages or create paid accounts.

## Choose a setup path

| Path | Runs on your computer | Sends to a hosted model | Best starting point |
| --- | --- | --- | --- |
| Native + local | Rust, UI, SQLite, Ollama, media tools and models | Nothing | Apple Silicon with native Ollama GPU support |
| Docker + local | App container, persistent volumes, and host or container Ollama | Nothing | Reproducible app environment on Linux/macOS/WSL2 |
| Native/Docker + Groq | App, SQLite, local embeddings, optional local vision/speech | Text, retrieved passages, PDF text and transcripts used by text-only turns | Faster text generation or a smaller optional-media setup |

Groq does not replace the embedding service in this implementation. Even the hosted-text path uses local Nomic embeddings for Research mode. Turns containing images select the independent local vision generator. `--skip-media` skips optional Whisper/vision downloads; it does not invent those capabilities or remove models already installed.

## Hardware and software

These are practical planning recommendations, not measured universal minimums. The full native multimedia path was tested on an Apple M3 with 16 GB RAM. Performance depends on model choice, context, competing applications and media length.

| Resource | Full local multimedia | Groq text with optional media skipped |
| --- | --- | --- |
| RAM | 16 GB recommended; more leaves room for development tools | 8 GB is a reasonable starting point for the app and local embeddings |
| CPU | Modern 4+ core CPU; supported GPU acceleration improves generation | Modern CPU; local embeddings still use resources |
| Free disk | Plan for 20 GB including models, build tools and Docker caches | Plan for 8–10 GB with tools/caches; less if only pulling a runtime image |
| Model download | About 4.2 GB for Gemma 3, Nomic and Whisper small | Nomic alone is a much smaller download; media adds the other models |
| Network | Needed for tools/models initially; local inference works afterward | Needed for Groq requests, plus initial setup |
| Accounts | GitHub for a fork; no cloud model account needed | A Groq account and your own API key |

Native development uses Rust 1.94.0, Node.js 22.22.2 (or compatible Node 22), npm, Python 3.10+, Git and Ollama. Full media additionally needs FFmpeg/ffprobe, Poppler's `pdfinfo`/`pdftotext`/`pdftoppm`, and `whisper-cli`. Docker setup uses Docker Engine/Desktop with Compose v2 plus Python and Git; Rust/Node/media tools are inside the image. Building Docker images requires additional temporary disk space.

The initial full application checks were on macOS. CI and container checks provide separate Linux evidence; see [verification](../verification.md) for the actual runs. Windows instructions assume WSL2 with Docker integration and a Linux shell; native Windows media binaries and PowerShell scripts are not separately validated.

## 1. Get the repository

```sh
git clone https://github.com/kaw393939/is373-rust-rag.git
cd is373-rust-rag
```

Fork it first if you intend to publish your own changes. Use your existing course project record for research and decisions. The repository contains code and a fictional teaching corpus; it does not contain model weights, user chat databases or API keys.

## 2A. Native prerequisites

Install Rust using [rustup](https://rustup.rs/), Node from [the Node project](https://nodejs.org/en/download), and [Ollama](https://ollama.com/download). On macOS, install media tools with Homebrew:

```sh
brew install ffmpeg whisper-cpp poppler
rustup toolchain install 1.94.0 --profile minimal --component rustfmt --component clippy
```

On Debian/Ubuntu, system packages can provide the compiler basics, FFmpeg and Poppler:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config curl ca-certificates cmake ffmpeg poppler-utils python3-venv
```

Install `whisper-cli` using the [official whisper.cpp build instructions](https://github.com/ggml-org/whisper.cpp). If it is not on PATH, export `WHISPER_CLI` as its absolute path before starting Rust. Do not confuse the `whisper-cli` executable with the separately downloaded Whisper model file. Node, Rust and Ollama installation are separate steps.

Start the Ollama application. If it is not already running, use `ollama serve` in another terminal. An “address already in use” message usually means a service is already running; check `ollama list` before starting another instance.

## 2B. Docker prerequisites

Install [Docker Desktop](https://docs.docker.com/get-started/get-docker/) or Docker Engine with the Compose plugin. Verify `docker version` shows both a client and a server and `docker compose version` succeeds. Start Docker Desktop before using the setup script.

On Apple Silicon, native host Ollama can use Metal acceleration while the Linux app container calls it through `host.docker.internal`. Ollama inside a Linux container on macOS does not gain Metal acceleration. On Linux/WSL2, the setup script defaults to the optional Compose Ollama service. The default Compose file does not configure NVIDIA GPU access; GPU passthrough is a separate tested extension.

## 3. Run the setup assistant

```sh
python3 scripts/setup.py
```

Choose `local` or `groq`, then `native` or `docker`. The Groq choice asks for a key through a hidden terminal prompt, checks model access and saves configuration in ignored `.env.local` with owner-only permissions. Do not paste the instructor's key into a student project. Each student should use their own permitted account and resource limits.

The assistant downloads configured models, builds or pulls the app, and imports the corpus. Expect progress output and a longer first run. It does not start an automatic recurring download job. Existing verified Whisper files are reused.

Explicit examples:

```sh
# Full native local experience:
python3 scripts/setup.py --provider local --runtime native --yes
# Published container with its own local Ollama service:
python3 scripts/setup.py --provider local --runtime docker --container-ollama --yes
# Groq text; the script prompts privately for the key:
python3 scripts/setup.py --provider groq --runtime native --skip-media
# Configure first without network/model downloads:
python3 scripts/setup.py --provider local --runtime docker --configure-only
```

For development before an image release, add `--build-image` to Docker setup. The regular Docker path pulls `kaw393939/is373-rust-rag:latest`; for reproducible work, set `ORDO_IMAGE` to a verified digest after setup. The setup script writes its own configuration; review intentional custom settings before rerunning it.

## 4. Start and verify

```sh
python3 scripts/start.py
```

Open http://127.0.0.1:3730. Native mode stays attached to the terminal; stop with Ctrl-C. Docker mode starts services in the background. Workspace settings reports model and media readiness. Ask the annual membership-fee question in Research mode and inspect the source. Then ask a question the corpus cannot answer.

```sh
curl -fsS http://127.0.0.1:3730/api/health
curl -fsS http://127.0.0.1:3730/api/status
```

Health means the HTTP service is alive. Status reports model/index/media readiness; it can be partially ready. Do not treat a healthy container as proof that weights or API credentials are installed.

## Everyday commands and persistence

```sh
# Docker status, logs and a normal stop; preserve volumes:
docker compose --env-file .env.local ps
docker compose --env-file .env.local logs --tail=100 app
docker compose --env-file .env.local down
# Re-import after a corpus or embedding-model change:
docker compose --env-file .env.local run --rm app import
```

Native users can run `cargo run --locked -- import` with the same exported configuration used by the server. `.env.example` documents settings; Cargo does not automatically load it or `.env.local`. `scripts/start.py` does load `.env.local`. For native frontend development, run Rust and `npm --prefix frontend run dev`; Vite proxies API traffic to Rust.

Restart after provider/model configuration changes. Changing the generator does not require re-embedding; changing the embedding model or revision does. Read [models](models.md), [delivery](delivery.md) and [troubleshooting](troubleshooting.md) before deleting data to solve a setup problem.

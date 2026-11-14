#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for binary in ollama whisper-cli ffmpeg ffprobe pdfinfo pdftotext pdftoppm curl; do
  command -v "$binary" >/dev/null || { echo "Missing $binary. See docs/course/setup.md." >&2; exit 1; }
done
bash scripts/download_whisper.sh
ollama pull gemma3:4b
echo 'Vision and speech models are installed; PDF/video tools are available. Restart Rust after configuration changes.'

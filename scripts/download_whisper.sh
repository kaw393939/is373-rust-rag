#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
whisper_variant=${WHISPER_VARIANT:-small}
case "$whisper_variant" in
  small) expected_sha=1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b ;;
  base) expected_sha=60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe ;;
  *) echo "WHISPER_VARIANT must be small or base" >&2; exit 1 ;;
esac
model_path=${WHISPER_MODEL:-data/models/ggml-$whisper_variant.bin}
mkdir -p "$(dirname "$model_path")"
checksum() {
  if command -v shasum >/dev/null; then shasum -a 256 "$1" | cut -d ' ' -f 1
  else sha256sum "$1" | cut -d ' ' -f 1; fi
}
if [[ ! -f "$model_path" ]]; then
  trap 'rm -f "$model_path.part"' EXIT
  curl -fL --retry 3 --max-time 600 \
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-$whisper_variant.bin" \
    -o "$model_path.part"
  [[ "$(checksum "$model_path.part")" == "$expected_sha" ]] || { echo "Whisper model checksum mismatch" >&2; exit 1; }
  mv "$model_path.part" "$model_path"
fi
[[ "$(checksum "$model_path")" == "$expected_sha" ]] || { echo "Unexpected Whisper model; move it aside before rerunning setup." >&2; exit 1; }
echo "Verified speech model: $model_path"

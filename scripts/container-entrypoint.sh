#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-serve}" == 'setup-models' ]]; then
  exec bash /app/scripts/download_whisper.sh
fi
exec /usr/local/bin/is373-rag "${@:-serve}"

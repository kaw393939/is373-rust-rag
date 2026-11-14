# Troubleshooting by failing layer

Start with the exact failing step and a small reproducible input. Capture the error, OS, app commit/image digest, role/model configuration and whether the failure occurs before or after the first answer delta. Never include keys, `.env.local`, private documents or full credential-bearing requests in an issue.

| Symptom | Check and response |
| --- | --- |
| `cargo`, `npm`, Python or media tool not found | Follow the native prerequisites in [setup](setup.md), or use the runtime container. A downloaded model file does not install its executable. |
| Docker client exists but cannot connect | Start Docker Desktop/Engine and inspect `docker version` and the selected context. |
| Ollama address already in use | Check `ollama list`; the app or another terminal may already run the service. Do not kill another task's service. |
| Health is OK, generation is unavailable | Inspect `/api/status` and workspace settings. Install the named model/key, verify Ollama connectivity, then retry. Health only tests HTTP. |
| Container cannot reach Ollama | Inside the container, `127.0.0.1` means that container. Use `http://ollama:11434` for Compose or `http://host.docker.internal:11434` for host Ollama. Host service listening/firewall policy must permit the connection. |
| Host or Origin rejected | Use localhost/127.0.0.1 and the mapped port. Do not disable the guard to expose the unauthenticated app publicly. Vite development uses port 5173. |
| Port 3730 occupied | Stop your own earlier server or choose another `PORT`; do not terminate unrelated processes. |
| No indexed sources / model revision mismatch | Run import with the same embedding provider/model used by the server. A changed digest requires re-embedding. |
| Groq HTTP 401/403 | Verify key validity, expiration and account/model access. This adapter supplies an explicit User-Agent; a bare scripted client received 403 during setup testing. Do not print the key to diagnose it. |
| Groq HTTP 429 / service failure | Inspect account limits/provider status; wait or reduce usage. No silent paid fallback is configured. |
| Local generation slow | Separate cold loading from warm latency, close unnecessary workloads, reduce input/context, and verify native GPU support. Docker on macOS does not give Ollama Metal access. |
| A response is marked incomplete | Check token limits, missing/invalid citations and stream termination. A plausible partial sentence is not a completed run. |
| Whisper unavailable | Check `whisper-cli`, FFmpeg and the model path/checksum. In Docker, run the setup-models command against the persistent app volume. |
| PDF fails before inference | Check encryption, corruption, page count (20), extracted text (16,000 characters) and rendered scan budget (4). Split the source deliberately; the app does not silently truncate it. |
| A PDF diagram is missing | Text-layer extraction preserves text, not every figure/layout. Pages with useful text are not automatically rasterized. Inspect extracted pages. |
| Video misses an event | Only 3 frames are sampled by default (configurable 1–4). Inspect timestamps. This is not continuous scene analysis. |
| Silent video / wrong transcript | `no_audio` is valid for a file without a track; `no_speech` differs from decoding failure. Compare the original speech with the transcript before changing the answer prompt. |
| Upload rejected | Check supported signature/MIME, 12 MB media limit, 24 KB text limit, two-minute duration, image dimensions and available 500 MB attachment quota. Renaming an extension does not convert a file. |
| An old transcript remains after changing Whisper | Historical messages intentionally preserve their original processing result. Reattach/reprocess to compare a new configuration. |
| Docker pull fails with an expired token | Refresh your Docker login using your own valid credential; do not commit credentials or overwrite a shared user's configuration without coordination. Public images can be pulled anonymously subject to registry limits. |

## Useful diagnostics

```sh
ollama list
docker compose --env-file .env.local ps
docker compose --env-file .env.local logs --tail=100 app
curl -fsS http://127.0.0.1:3730/api/health
curl -fsS http://127.0.0.1:3730/api/status
```

Review logs before sharing them. Never run `env`, print configuration files or paste an authorization header into a public issue. Archives retain conversations and files; they are not deletion. Do not delete volumes/databases as the first fix. Back up and test with a separate disposable database when investigating storage.

An npm audit currently reports low-severity development-tool advisories rooted in deepmerge without an automated fix. Record and review dependency findings; do not force incompatible upgrades merely to make the count disappear. Build dependencies are separate from the served UI, but should still be maintained.

# Evaluation: measure each stage

A polished answer is the last observable step of a longer process. Evaluate the evidence path before judging style. Preserve inputs, configuration, retrieved IDs, actual output and terminal state. Do not replace a failed result with a later passing one without keeping the earlier record.

| Stage | Question to ask | Useful evidence |
| --- | --- | --- |
| Extraction | Did the file yield the right page text, frames and audio? | Page boundaries, rendered images, timestamps |
| Recognition | Did vision or speech recognition identify the input correctly? | Original media beside transcript/model observation |
| Retrieval | Was the needed evidence selected? | Expected IDs versus retrieved IDs; missing/distracting passages |
| Generation | Does each claim follow from the available evidence? | Claim-by-claim comparison and appropriate abstention |
| Citation | Is the ID allowed, and does that source support that claim? | ID validation plus human source review |
| Delivery | Did the stream finish honestly and persist correctly? | Terminal event, partial state, reloaded conversation |

## A compact evaluation set

Use the five fictional corpus questions in [evaluate.py](../../scripts/evaluate.py). They cover an exact fee, a paraphrase, a relationship, visitor sign-in and an unannounced meeting date. For each question write expected evidence before looking at generated prose. Accept equivalent wording when the facts and support are correct. Include an unanswerable question so a model cannot score well by always guessing.

Retrieval recall asks whether the required evidence is in the selected set. Precision asks how much selected evidence is useful. This tiny corpus supports direct manual inspection; it is not a benchmark of broad knowledge. Latency should distinguish a cold model load from a warm request and record the hardware and competing workload.

## Run the available checks

```sh
cargo test --locked
npm --prefix frontend test
python3 scripts/test_setup.py
python3 scripts/check_docs.py
# Server and required local models must be running:
python3 scripts/check_chat.py
python3 scripts/evaluate.py
```

Media checks use synthetic fixtures, not student/private recordings. See [multimedia](../multimedia.md) for fixture generation and `check_multimedia.py` / `check_documents_video.py`. Fixture generation currently needs ReportLab, Pillow, pypdf, FFmpeg and macOS `say`; that speech generator is a macOS-specific development convenience. The application and container smoke checks do not require macOS. The native decoder test is normally ignored because it depends on tools/fixtures; run it explicitly when available:

```sh
cargo test --locked real_document_and_video_decoding -- --ignored
python3 scripts/check_container.py is373-rust-rag:local
```

`check_groq.py` expects a separately configured Groq server on port 3732 and the workshop PDF fixture. It sends real API requests and can consume account quota. It reads no key itself; the server owns the credential. Unit tests and CI need no paid model credentials.

## Learn from the retained failures

The [verification record](../verification.md) deliberately preserves a local answer whose citation ID exists but supports a different claim. Automated checks passed, while manual review passed only four of five answers. This is why reference syntax is not a factuality judge.

Whisper base heard “cedar” as “seater” in a synthetic video. The answer model repeated the transcript correctly; the error was recognition. Whisper small improved that fixture, but a one-example result is not a universal accuracy claim. Initial Groq output used decorative Unicode citation brackets; the reference guard rejected the missing required ASCII citation. A more explicit instruction produced valid IDs in subsequent tests. Prompt improvements need repeat evaluation because generation remains variable.

Do not claim production readiness from unit tests alone. The current checks do not establish arbitrary-document accuracy, accessibility conformance, multi-user security, proxy streaming behavior, sustained concurrency or recovery from every storage failure. Add tests when the corresponding capability is implemented and make the test's boundary visible.

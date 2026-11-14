# IS373 student guide: understand, run, inspect, improve

This is a working instructor reference for a local RAG application with a Rust server and the Ordo chat interface. It demonstrates the complete path from permitted source material to stored representations, retrieved evidence, streamed model output and a saved conversation. It also demonstrates local PDF, image, audio and sampled-video processing.

The [official individual RAG brief](https://github.com/kaw393939/is373-studio/blob/main/docs/projects/individual.md) and Canvas remain authoritative. This guide adds no assignment, due date, required language, paid service or grading rule. Students can transfer the same ideas to their existing stack. The guide now includes a separate authenticated hosted deployment; your own project requirements still come from the official brief.

## Start with the path that matches your goal

| Goal | Read / do |
| --- | --- |
| Run the app for the first time | [Requirements and installation](setup.md) |
| Understand the technologies and their jobs | [Technology foundations](technology.md) |
| Trace a question across the whole application | [Architecture and request lifecycle](architecture.md) |
| Understand the ETL pipelines and storage | [ETL and data lineage](etl.md) |
| Install models or configure a Groq API key | [Models, providers and credentials](models.md) |
| Understand Docker and the release process | [Containers and CI/CD](delivery.md) |
| Understand accounts and private data | [Authentication and authorization](authentication.md) |
| Build conversational administration | [Tool registry, prompts and feature flags](control-plane.md) |
| Deploy and operate a hosted instance | [Production deployment and recovery](production.md) |
| Learn through bounded experiments | [Guided labs](labs.md) |
| Decide whether an answer is supported | [Evaluation](evaluation.md) |
| Build an extension without coupling everything | [Extension guide](extensions.md) |
| Diagnose a failed setup or request | [Troubleshooting](troubleshooting.md) |
| Lead a workshop | [Instructor guide](teaching.md) |

## What you should be able to explain

After working through the guide, you should be able to follow one source from its manifest entry to a chunk, an embedding, a database row, retrieved evidence and a cited answer. You should be able to distinguish extraction errors from retrieval errors, recognition errors, generation errors and presentation errors. You should also explain why an API key belongs on the server, why changing an embedding model requires rebuilding the index, and why a successfully streamed answer can still be wrong.

You do not need to memorize the repository. Use the module map and follow the data structures. Read a small code path, predict its behavior, run it, and compare what happened with the prediction.

## What works and what remains an extension

Working: local text/vision generation, independent speech recognition, real vector search, keyword comparison, persistent chat, source cards, cancellation, PDF text/scanned pages, sampled video frames, transcripts, Groq text/vision/speech adapters, and local CPU embeddings. The hosted profile adds invitation-only accounts, private account storage and governed administration. The app has separate interfaces for generation, embedding, transcription, document extraction and video decoding.

Scope boundaries: one inference request at a time; no durable ingestion queue, scheduled synchronization or automatic promotion of chat uploads into the shared library. OpenAI/Anthropic/DeepSeek remain future adapters. Billing, organizations, MFA/SSO and automated retention/deletion are backlog items. Published images and deployment checks provide evidence for the specific tested profile, not an enterprise security or uptime guarantee.

## A useful learning sequence

1. Run the text-only path and inspect one supported answer plus an unanswerable question.
2. Read the manifest and trace a source through the importer.
3. Compare keyword and vector retrieval before changing model prompts.
4. Observe SSE events, a normal completion and an interrupted response.
5. Add multimedia, inspect the exact extracted inputs, and check the answer against them.
6. Change one bounded design choice and preserve both the earlier and later results.
7. Follow a commit through CI to tested images, deployment gates and a backup.
8. Trace an invitation, a denied cross-account request and a reviewed administrative command.

Use your existing project record or issue workflow for observations and peer feedback. The [experiment template](experiment-template.md) is optional scaffolding, not another required report.

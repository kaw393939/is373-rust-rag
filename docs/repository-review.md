# Course repository review · November 13, 2026

Reviewed the current published files in the user's GitHub repositories, rather than assuming the research architecture was already implemented.

| Repository / material | Finding | Decision here |
| --- | --- | --- |
| [is373-rag README](https://github.com/kaw393939/is373-rag) | Documentation, synthetic labs and a static Svelte welcome scaffold; research runtime planned | Build a separate local runnable slice; do not relabel the research runtime as finished |
| [Individual RAG brief in is373-studio](https://github.com/kaw393939/is373-studio/blob/main/docs/projects/individual.md) | About five permitted documents/questions; actual vectors or graph storage; real generation, provenance, baseline comparison and hosted evidence | Five corpus documents, SQLite vector index, Ollama, evidence cards and live evaluation; explicitly leave hosting for later |
| [Student guide](https://github.com/kaw393939/is373-rag/blob/main/docs/students/index.md) | Explain import → represent → retrieve → generate → check; original brief/Canvas authoritative | Make the pipeline visible, keep setup small and source text inspectable |
| [Optional retrieval lab](https://github.com/kaw393939/is373-rag/blob/main/student-reference/README.md) | Python baseline with five fictional Harbor Club policies and optional Ollama models | Reuse the same documents, model choices and question domain; implement a Rust HTTP server and genuine sqlite-vec index |
| [Streaming proposal](https://github.com/kaw393939/is373-rag/blob/main/docs/project/streaming.md) | Proposed Svelte/Rust architecture with explicit lifecycle, Unicode-safe decoding, cancellation and durable runs | Reuse event vocabulary and honest terminal outcomes; document that this local slice has no durable/replayable jobs |
| [is373_fall2026 README](https://github.com/kaw393939/is373_fall2026) | Portfolio starter, discovery and explaining AI-assisted choices; separate from later content systems | Treat this RAG app as its own technical product rather than changing portfolio requirements |

Repository discovery also found AI Chat, CI/CD, system-map and archived trial repositories. Those were not audited file-by-file in this first slice; the direct RAG guide and authoritative individual brief define its scope.

## Vector choice versus graph alternative

Vector similarity fits this first collection because users ask paraphrased questions about short policy passages. SQLite with sqlite-vec needs no separate database daemon and preserves sources, metadata and vector storage in one local file. Exact top-four search is sufficient for ten passages. Ollama supplies real embeddings rather than simulated numbers.

A graph alternative could represent `Events Committee → organizes → workshop` and `Membership Committee → reviews → application`, with each relationship backed by paragraph IDs. That would help precise relationship/path queries, but requires entity normalization and relationship extraction/maintenance. It is a valid alternative, not implemented here. Students should still research both and justify the path they select.

## Bounded experiment

Compare keyword counts with vector retrieval for the exact-fee question and “Who puts together the club's learning sessions?” Preserve actual candidates in `data/evaluation.json`. Inspect whether vector search recovers the workshop passage despite wording differences. Use the same generation prompt and corpus so the retrieval method is the changed variable. A later chunking experiment can compare paragraph chunks with overlapped windows, recording source offsets and re-embedding each candidate design.

## Still needed for the student hosted release

Persistent deployment with protected provider configuration and import permissions; repeatable hosting checks; evaluation of unsupported and adversarial questions; peer feedback and a tested improvement; documentation of the selected database and model versions. The Rust choice is an instructor example, not a course-wide requirement. That was the initial local-review scope. The reference is now prepared for its own public `is373-rust-rag` repository with student documentation and a container publishing workflow; these additions do not complete a student's hosted submission.

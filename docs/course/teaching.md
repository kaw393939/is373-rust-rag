# Instructor workshop guide

Use the [official IS373 individual brief](https://github.com/kaw393939/is373-studio/blob/main/docs/projects/individual.md) and Canvas for requirements and assessment. This reference explains one implementation. Rust, Svelte, Docker, Groq and the multimedia features are examples, not added course mandates. The local reference does not satisfy the later hosted-operation requirement on a student's behalf.

## Prepare a demonstration

Run [setup](setup.md) on the workshop machine in advance. Download models and verify one text answer before class; first model loads can be slow. Keep an offline local path available when network access is uncertain. Use synthetic or permitted media. Have students obtain their own permitted API keys if they choose hosted generation; do not distribute an instructor credential.

Demonstrate the source drawer and extracted inputs before showing visual polish. A supported fee answer, an unsupported date question and the retained citation-support failure make a useful comparison. Display the actual evidence and let students decide whether each claim follows from it.

## Suggested 90-minute sequence

| Time | Activity | Understanding to check |
| --- | --- | --- |
| 0–15 minutes | Run a question; inspect source and saved answer | Where did the claim come from? |
| 15–30 | Trace manifest → chunks → vectors → retrieval | What is stored, and what invalidates it? |
| 30–45 | Compare vector/keyword retrieval on a paraphrase | Is the failure retrieval or generation? |
| 45–60 | Inspect PDF text, video samples and transcript | Which observations did the model actually receive? |
| 60–75 | Trace SSE and cancellation; inspect provider boundary | How do we distinguish completion from disconnect? |
| 75–90 | Follow a commit through CI to an image | What is reproducible, persistent and still untested? |

This is an adaptable workshop plan, not a deadline or graded submission specification. Use [labs](labs.md) for longer sessions and [experiment template](experiment-template.md) only if it helps the existing project record.

## Connect to the existing assignment

The fictional five-document corpus demonstrates permitted sources, source/version provenance, retrieval and cited generation. The keyword comparator helps explain why the vector approach was chosen. Students should still research the assignment's vector and graph alternatives and justify their own choice; this implementation does not substitute for that reasoning.

The retained baseline and one-variable improvements illustrate a bounded experiment. Have students state the problem, prediction, evidence and resulting tradeoff in their existing record. Peer feedback should identify a reproducible issue or useful improvement in that same workflow, rather than creating redundant reports.

Ask students to explain a request in their own words, locate the owning module, predict a failure and demonstrate it with a safe fixture. Questions such as “Why does changing the embedding model require import again?” and “How can a valid citation be wrong?” reveal understanding more clearly than a screenshot of a polished answer.

Before using a hosted production deployment as a teaching example, implement and test the [operational boundaries](extensions.md). Explain honestly what the demonstration proves and what remains a design proposal.

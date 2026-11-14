# Guided labs

These are optional learning activities, not new grading requirements. Use your existing project record. For every activity: predict, run, inspect evidence, explain the difference, then change one thing. Keep credentials and personal data out of screenshots and commits.

## 1. Trace an answer

Run the app and select Research. Ask “How much is the annual membership fee?” Open the source drawer and compare the answer with the exact passage. Find `membership` in `corpus/manifest.json` and then the named source file. Follow the paragraph into [store.rs](../../src/store.rs), and the request handler in [main.rs](../../src/main.rs).

Record the source ID, paragraph ID, retrieved text and cited claim. Explain why a real citation ID does not establish that a claim is true. Ask “When is next year's annual meeting?” and check whether the source actually provides a date. A plausible invented date is a failure even if the sentence has a citation.

## 2. Inspect ETL and repeatability

Read [ETL](etl.md). Run import twice with unchanged sources and the same embedding model. Expect the second import to report unchanged. In your own branch, change one fictional source and its recorded version, then import again. Check that retrieval shows the new content and no stale paragraph remains. Restore your source and import again.

Explain the difference between a source version, a content hash and a model digest. Predict what should happen if embedding generation fails halfway through a rebuild. Read the import rollback test before testing with real data. Never simulate failure by corrupting your only database.

## 3. Compare retrieval before judging prose

Use “Who organizes learning sessions?” with vector and keyword retrieval. Compare selected IDs and passages before comparing generated answers. Explain why a semantic embedding may connect “learning sessions” with “workshops” while literal matching misses it.

Build a five-question set containing an exact fact, a paraphrase, a relationship, a detail with a plausible distractor, and an unsupported question. Keep expected evidence separate from expected answer wording. Change one retrieval choice, such as top-k, in a branch; record the tradeoff between evidence coverage and distractor context. More retrieved text is not automatically better.

## 4. Observe streaming and interruption

Open browser developer tools → Network and submit a longer Chat response. Inspect the `/api/chat` response: events describe application progress and answer deltas. Stop generation, reload, and inspect the saved partial answer's terminal state. Read [stream.js](../../frontend/src/stream.js) and the Rust stream/persistence code.

Explain why a closed connection is not proof of success, why UTF-8 bytes may arrive in fragments, and why each provider maps its finish reason into a shared contract. Run the frontend stream tests and identify which failure each test prevents.

## 5. Separate extraction, recognition and answering

Upload a small permitted PDF with text on two pages. Ask for one fact on page two and inspect the extracted page. Then try a scanned page with a clearly printed word. Explain why the first uses text extraction and the second uses image understanding.

Upload a short video with visibly different beginning/middle/end frames and a spoken sentence. Inspect the sampled timestamps and saved transcript before judging the answer. Identify an event that might happen between samples and therefore be missed. Do not claim the application watched every frame. Compare one transcript under Whisper base and small while keeping the input unchanged; preserve both results.

## 6. Change one provider role

Read [models](models.md). With your own Groq key, choose hosted text through setup. Compare the same library question locally and through Groq. Inspect status and the saved model name. Then add an image: explain why this selects the independently configured local vision generator.

Draw exactly which data leaves the machine on each path. Explain why local PDF extraction does not imply that extracted PDF text stays local when hosted text generation is selected. Do not use private material for this exercise.

## 7. Follow source to image

In a fork with your own registry configuration, open a small pull request, inspect checks and fix a deliberate safe formatting failure. After merging a reviewed change, follow its commit through both architecture builds to the shared image manifest. Run the exact digest and compare it with the source revision label.

Explain which files live in the image, which in volumes, which in the browser, and which are secrets. Stop and recreate the container without deleting its volumes; verify a conversation persists. Use [delivery](delivery.md) to explain why publication and production deployment are different milestones.

# Contributing

Start with the [student guide](docs/course/README.md) and use a branch for a bounded change. Describe the concrete problem, resulting behavior and how you verified it. Keep failed baseline observations when comparing model changes; a model's plausible answer is not enough evidence.

Run `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, frontend tests/build, `python3 scripts/test_setup.py` and `python3 scripts/check_docs.py`. For runtime packaging changes, build and run `scripts/check_container.py`. For inference changes, run the relevant live checks with your own configured models/credentials and manually review source support.

Do not commit `.env.local`, keys, databases, model weights, uploads or private screenshots. Add only source material you have permission to redistribute and preserve its provenance. Provider integrations must keep credentials server-side and accurately report capabilities and data destinations.

This repository is an instructor reference; the official course brief and Canvas remain authoritative. Propose changes through the repository's normal pull-request/issue workflow without introducing additional student requirements.

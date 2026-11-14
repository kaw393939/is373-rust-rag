"""Live, local evaluation. Run the server first. Saves actual evidence, not canned results."""
import json
import re
import time
import urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:3730"
CASES = [
    ("exact", "How much is the annual membership fee?", "membership:p2", "$15"),
    ("paraphrase", "Who puts together the club's learning sessions?", "events:p2", "Events Committee"),
    ("relationship", "Does the secretary approve membership applications?", "committee:p3", "does not approve"),
    ("procedure", "Can visitors attend public workshops?", "events:p3", "welcome desk"),
    ("unanswerable", "What is the exact date of next year's annual meeting?", "events:p3", None),
]

def post(path, data):
    return urllib.request.urlopen(urllib.request.Request(BASE + path, json.dumps(data).encode(), {"Content-Type": "application/json"}), timeout=180)

results = []
for kind, question, expected, phrase in CASES:
    started = time.monotonic()
    with post("/api/search", {"question": question, "mode": "keyword"}) as r:
        baseline = json.load(r)["sources"]
    events, answer, evidence, outcome = [], "", [], None
    first_event_ms = None
    with post("/api/chat", {"question": question, "mode": "vector"}) as r:
        name = None
        for raw in r:
            line = raw.decode().strip()
            if line.startswith("event:"): name = line[6:].strip()
            if not line.startswith("data:"): continue
            data = json.loads(line[5:])
            if first_event_ms is None: first_event_ms = round((time.monotonic()-started)*1000)
            events.append({"event": name, "data": data})
            if name == "answer.delta": answer += data["text"]
            if name == "retrieval.evidence_selected": evidence = data["sources"]
            if name == "run.ended": outcome = data["outcome"]
    ids = [c["id"] for c in evidence]
    citations = re.findall(r"\[([a-z]+:p\d+)\]", answer)
    expected_text = phrase.lower() in answer.lower() if phrase else any(s in answer.lower() for s in ["no date", "not been announced", "don't have enough", "do not have enough", "not available", "does not provide", "doesn't provide", "not provide", "not specified"])
    checks = {"expected_evidence": expected in ids, "expected_answer_marker": expected_text,
              "citations_resolve": (bool(citations) or kind == "unanswerable") and all(c in ids for c in citations), "completed": outcome == "complete"}
    results.append({"kind": kind, "question": question, "expected_chunk": expected, "answer": answer,
                    "checks": checks, "vector_sources": evidence, "keyword_source_ids": [c["id"] for c in baseline],
                    "first_event_ms": first_event_ms, "elapsed_seconds": round(time.monotonic()-started,2), "events": events})
    print(kind, checks, flush=True)
with urllib.request.urlopen(BASE + "/api/status") as r: config = json.load(r)
Path("data/evaluation.json").write_text(json.dumps({"configuration":config,"results":results},indent=2)+"\n")
print("Saved data/evaluation.json. Automated markers do not establish factual citation support; review answers against passages.")
raise SystemExit(0 if all(all(r["checks"].values()) for r in results) else 1)

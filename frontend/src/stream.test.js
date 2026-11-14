import { test } from 'node:test';
import assert from 'node:assert/strict';
import { consumeEvents } from './stream.js';
function body(text) { const bytes = new TextEncoder().encode(text); return new ReadableStream({ start(c) { for (const b of bytes) c.enqueue(new Uint8Array([b])); c.close(); } }); }
test('one-byte fragments preserve Unicode and terminal outcome', async () => {
  const events=[];
  await consumeEvents(body('event: run.started\ndata: {}\n\n: ping\n\nevent: answer.delta\ndata: {"text":"café 🦀"}\n\nevent: run.ended\ndata: {"outcome":"complete"}\n\n'), (n,d)=>events.push([n,d]));
  assert.equal(events[1][1].text,'café 🦀'); assert.equal(events[2][1].outcome,'complete');
});
test('EOF without terminal event is a failure',async()=> {
  await assert.rejects(consumeEvents(body('event: run.started\ndata: {}\n\n'),()=>{}),/interrupted/);
});
test('delta before start is rejected',async()=> {
  await assert.rejects(consumeEvents(body('event: answer.delta\ndata: {"text":"hi"}\n\n'),()=>{}),/lifecycle/);
});
test('media preparation keeps the lifecycle open until generation finishes',async()=> {
  const events=[];
  await consumeEvents(body('event: run.started\ndata: {}\n\nevent: input.progress\ndata: {"stage":"transcribing"}\n\nevent: input.ready\ndata: {"attachments":[]}\n\nevent: run.ended\ndata: {"outcome":"failed"}\n\n'),(n,d)=>events.push([n,d]));
  assert.equal(events[1][1].stage,'transcribing');
  assert.equal(events.at(-1)[1].outcome,'failed');
});

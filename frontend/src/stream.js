// Decode bytes incrementally; a closed socket is never proof of completion.
export async function consumeEvents(body, onEvent) {
  const reader = body.getReader();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  let buffer = '', started = false, terminal = false;
  try {
    while (true) {
      const { value, done } = await reader.read();
      buffer += decoder.decode(value, { stream: !done });
      if (buffer.length > 262144) throw new Error('Stream frame exceeds limit.');
      let boundary;
      while ((boundary = buffer.indexOf('\n\n')) !== -1) {
        const frame = buffer.slice(0, boundary); buffer = buffer.slice(boundary + 2);
        const name = frame.split('\n').find(l => l.startsWith('event:'))?.slice(6).trim();
        if (!name) continue; // Keepalive comments.
        const raw = frame.split('\n').filter(l => l.startsWith('data:')).map(l => l.slice(5).trimStart()).join('\n');
        const data = JSON.parse(raw);
        if (terminal || (!started && name !== 'run.started')) throw new Error('Invalid stream lifecycle.');
        if (name === 'run.started') { if (started) throw new Error('Duplicate start.'); started = true; }
        else if (!['input.progress', 'input.ready', 'retrieval.evidence_selected', 'answer.delta', 'run.ended'].includes(name)) throw new Error('Unknown stream event.');
        if (name === 'run.ended') {
          if (!['complete', 'incomplete', 'refused', 'failed', 'interrupted'].includes(data.outcome)) throw new Error('Unknown completion outcome.');
          terminal = true;
        }
        onEvent(name, data);
      }
      if (done) break;
    }
    if (!terminal || buffer.trim()) throw new Error('Connection interrupted before confirmed completion.');
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
}

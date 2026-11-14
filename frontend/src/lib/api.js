export async function api(path, options = {}) {
  const response = await fetch(`/api${path}`, options.body instanceof FormData ? options : {
    ...options, headers: { 'Content-Type': 'application/json', ...options.headers },
    ...(options.body ? { body: JSON.stringify(options.body) } : {})
  });
  if (!response.ok) {
    if(response.status===401&&!path.startsWith('/auth/'))window.dispatchEvent(new Event('ordo:unauthorized'));
    let error; try { error = (await response.json()).error; } catch { /* HTTP fallback */ }
    throw new Error(error || `Request failed (${response.status}). Please try again.`);
  }
  return response.json();
}
export const formatSize = size => size < 1024 * 1024 ? `${Math.max(1, Math.round(size / 1024))} KB` : `${(size / 1024 / 1024).toFixed(1)} MB`;
export const mediaAssetUrl = (file,id) => `/api/attachments/${encodeURIComponent(file.id)}/assets/${encodeURIComponent(id)}`;
export const mediaUrl = file => file.asset_id ? mediaAssetUrl(file,file.asset_id) : `/api/attachments/${encodeURIComponent(file.id)}`;
export function attachmentText(a) {
  const pdf=a.document ? '\n'+a.document.pages.map(p=>`PDF page ${p.page}: ${p.text||'[Rendered page supplied to vision]'}`).join('\n\n')+'\n'+a.document.warnings.join('\n') : '';
  const video=a.video ? `\nVideo: ${a.video.duration_seconds.toFixed(1)}s; frames sampled near ${a.video.frames.map(f=>f.timestamp_seconds.toFixed(2)+'s').join(', ')}; audio: ${a.video.audio_state}.\n${a.video.warnings.join('\n')}` : '';
  return `Attachment: ${a.name} (${a.mime})${pdf}${video}${a.transcript ? `\nTranscript (${a.transcript.language}): ${a.transcript.text}` : ''}`;
}
export function exportConversation(conversation) {
  const text = `# ${conversation.title}\n\n` + conversation.messages.map(m => `## ${m.role === 'user' ? 'You' : 'Assistant'}\n\n${m.content}\n\n${m.attachments.map(attachmentText).join('\n')}${m.evidence.length ? '\n\nSources:\n' + m.evidence.map(e => `- [${e.id}] ${e.title}: ${e.origin}`).join('\n') : ''}`).join('\n\n---\n\n');
  const url = URL.createObjectURL(new Blob([text], {type:'text/markdown'}));
  const a = document.createElement('a'); a.href=url; a.download=`${conversation.title.replace(/[^a-z0-9 -]/gi,'').slice(0,70)||'conversation'}.md`; a.click();
  setTimeout(()=>URL.revokeObjectURL(url),1000);
}

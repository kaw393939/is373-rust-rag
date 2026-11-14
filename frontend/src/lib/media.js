export function mediaLabel(file, capabilities = {}, submitted = false) {
  if (file.input_state === 'processing') return file.mime === 'application/pdf' ? 'Extracting pages…' : file.mime.startsWith('video/') ? 'Processing video…' : file.mime.startsWith('audio/') ? 'Transcribing…' : 'Preparing…';
  if (file.input_state === 'failed') return 'Could not process';
  if (file.document) return `${file.document.pages.length} pages included`;
  if (file.video) return `${file.video.frames.length} frames${file.transcript ? ' + transcript' : ' included'}`;
  if (file.transcript) return 'Transcript included';
  if (file.mime === 'application/pdf') return submitted ? 'PDF not processed' : capabilities.pdf_input ? 'Page extraction' : 'PDF processing unavailable';
  if (file.mime.startsWith('video/')) return submitted ? 'Video not processed' : capabilities.video_input ? 'Frames + speech on send' : 'Video understanding unavailable';
  if (submitted && file.mime.startsWith('audio/')) return 'Transcript not generated';
  if (file.mime === 'text/plain') return 'Included as text';
  if (file.mime.startsWith('image/')) return capabilities.image_input ? 'Image understanding' : 'Vision unavailable';
  if (file.mime.startsWith('audio/')) return capabilities.audio_input ? 'Transcribed on send' : 'Transcription unavailable';
  return 'Preview only';
}
export function mediaHint(files, capabilities = {}) {
  if (files.some(f=>f.mime==='application/pdf') && !capabilities.pdf_input) return 'PDF extraction needs setup. Check Model & capabilities in settings.';
  if (files.some(f=>f.mime.startsWith('video/')) && !capabilities.video_input) return 'Video understanding needs setup. Check Model & capabilities in settings.';
  if (files.some(f=>f.mime.startsWith('video/'))) return 'Videos: up to 2 minutes. Sampled frames + speech; events between frames may be missed.';
  if (files.some(f=>f.mime==='application/pdf')) return 'PDFs: up to 20 pages. Text is extracted; scanned pages use vision. Eight visual inputs per message.';
  if (files.some(f => f.mime.startsWith('image/')) && !capabilities.image_input) return 'The vision model is unavailable. Check Model & capabilities in settings.';
  if (files.some(f => f.mime.startsWith('audio/')) && !capabilities.audio_input) return 'Transcription needs setup. Check Model & capabilities in settings.';
  if (files.some(f => f.mime.startsWith('audio/'))) return `${capabilities.audio_local?'Speech is transcribed on this machine.':'Speech is sent to the configured transcriber.'} Up to 2 minutes per recording.`;
  if (files.some(f => f.mime.startsWith('image/'))) return `Images are read by the ${capabilities.image_local?'local':'configured'} vision model. GIFs use the first frame.`;
  return 'Text attachments are included in the conversation.';
}

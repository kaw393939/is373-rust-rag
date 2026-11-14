<script>
  import {FileText, Download, Music2, Film, X, Image} from 'lucide-svelte';
  import {mediaUrl,formatSize,mediaAssetUrl} from '../lib/api.js';
  import {mediaLabel} from '../lib/media.js';
  export let file; export let compact=false;export let onremove=null;export let onpreview=()=>{};export let capabilities={};
</script>
<div class:compact class="attachment-card">
  {#if file.mime.startsWith('image/')}
    <button class="image-preview" onclick={()=>onpreview(file)} aria-label={`Preview ${file.name}`}><img src={mediaUrl(file)} alt={file.name} loading="lazy"/></button>
  {:else if !compact && file.mime.startsWith('audio/')}
    <div class="media-playback"><Music2 size={20}/><audio controls preload="metadata" src={mediaUrl(file)} aria-label={file.name}></audio></div>
  {:else if !compact && file.mime.startsWith('video/')}
    <!-- svelte-ignore a11y_media_has_caption (user-supplied media has no caption track) -->
    <video controls preload="metadata" src={mediaUrl(file)} aria-label={file.name}></video>
  {:else}<div class="file-symbol">{#if file.mime.startsWith('audio/')}<Music2 size={20}/>{:else if file.mime.startsWith('video/')}<Film size={20}/>{:else}<FileText size={20}/>{/if}</div>{/if}
  <div class="attachment-caption"><span title={file.name}>{file.name}</span><small class:media-failed={file.input_state==='failed'}>{formatSize(file.size)} · {mediaLabel(file,capabilities,!compact)}</small></div>
  {#if !compact && file.document}<details class="processed-input"><summary>View PDF content <span>{file.document.pages.length} pages</span></summary><div class="extracted-pages">{#each file.document.pages as page}<section><strong>Page {page.page}</strong>{#if page.image_id}<button class="processed-preview" aria-label={`Preview ${file.name} page ${page.page}`} onclick={()=>onpreview({...file,mime:'image/jpeg',asset_id:page.image_id,name:`${file.name} · page ${page.page}`})}><img src={mediaAssetUrl(file,page.image_id)} alt={`Rendered page ${page.page}`} loading="lazy"/></button><small>Rendered for vision · no reliable text layer</small>{/if}{#if page.text}<pre>{page.text}</pre>{/if}</section>{/each}</div><div class="processing-notes">{#each file.document.warnings as note}<p>{note}</p>{/each}</div></details>{/if}
  {#if !compact && file.video}<details class="processed-input"><summary>View sampled frames <span>{file.video.frames.length} frames</span></summary><div class="frame-grid">{#each file.video.frames as frame,i}<button aria-label={`Preview video frame ${i+1} near ${frame.timestamp_seconds.toFixed(1)} seconds`} onclick={()=>onpreview({...file,mime:'image/jpeg',asset_id:frame.image_id,name:`${file.name} · frame ${i+1} near ${frame.timestamp_seconds.toFixed(2)}s`})}><img src={mediaAssetUrl(file,frame.image_id)} alt={`Video frame ${i+1}`} loading="lazy"/><span>≈ {frame.timestamp_seconds.toFixed(1)}s</span></button>{/each}</div><div class="processing-notes"><p>{file.video.duration_seconds.toFixed(1)}s clip · {file.video.audio_state==='transcribed'?'Speech transcribed':file.video.audio_state==='no_audio'?'No audio track':'No speech recognized'}</p>{#each file.video.warnings as note}<p>{note}</p>{/each}</div></details>{/if}
  {#if !compact && file.transcript}<details class="transcript"><summary>View transcript <span>{Math.round(file.transcript.duration_seconds)}s · {file.transcript.language}</span></summary><p>{file.transcript.text}</p><small>{file.transcript.model} · Check speech recognition for errors.</small></details>{/if}
  {#if !compact && file.input_state==='failed'}<p class="attachment-error">{file.input_note}</p>{/if}
  {#if onremove}<button class="remove-attachment" onclick={()=>onremove(file.id)} aria-label={`Remove ${file.name}`}><X size={13}/></button>{:else}<a class="attachment-download" href={mediaUrl(file)} download={file.name} aria-label={`Download ${file.name}`}><Download size={14}/></a>{/if}
</div>

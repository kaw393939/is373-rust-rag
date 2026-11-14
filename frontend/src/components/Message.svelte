<script>
  import {Copy,Check,RotateCcw,Pencil,BookOpen,AlertCircle,ArrowUpRight} from 'lucide-svelte';
  import RichText from './RichText.svelte';
  import Attachment from './Attachment.svelte';
  export let message; export let active=false;export let onretry=()=>{};export let onedit=()=>{};export let onsources=()=>{};export let onpreview=()=>{};
  export let capabilities={};
  let copied=false,copyError=false;
  async function copy(){try{await navigator.clipboard.writeText(message.content);copied=true;setTimeout(()=>copied=false,1800);}catch{copyError=true;}}
</script>
<article class:user-message={message.role==='user'} class:assistant-message={message.role==='assistant'} class="message" aria-label={message.role==='user'?'Your message':'Assistant response'}>
  {#if message.role==='user'}
    <div class="user-content">
      {#if message.attachments?.length}<div class="message-attachments">{#each message.attachments as file}<Attachment {file} {onpreview} {capabilities}/>{/each}</div>{/if}
      <div class="user-bubble">{message.content}</div>
      <div class="message-actions user-actions"><button title="Edit in a new branch" aria-label="Edit message in a new branch" disabled={active} onclick={()=>onedit(message)}><Pencil size={14}/></button><button title="Copy message" aria-label="Copy message" onclick={copy}>{#if copied}<Check size={14}/>{:else}<Copy size={14}/>{/if}</button></div>
    </div>
  {:else}
    <div class="assistant-avatar" aria-hidden="true"><span class="logo-glyph">✳</span></div>
    <div class="assistant-content"><div class="assistant-byline"><strong>Ordo</strong><span>{message.model?.replace(':latest','')||'Assistant'}</span>{#if active}<span class="live-label" role="status"><i></i> {message.content?'Writing':message.stage||'Thinking'}</span>{/if}</div>
      {#if message.content}<RichText text={message.content}/>{:else if active}<div class="thinking" aria-label="Preparing response"><i></i><i></i><i></i></div>{/if}
      {#if message.evidence?.length}<button class="sources-pill" onclick={()=>onsources(message)}><span class="source-stack"><BookOpen size={12}/></span>{message.evidence.length} sources <ArrowUpRight size={13}/></button>{/if}
      {#if message.notice}<div class="message-notice"><AlertCircle size={15}/><span>{message.notice}</span></div>{/if}
      {#if !active}<div class="message-actions"><button onclick={copy} title="Copy response" aria-label="Copy response">{#if copied}<Check size={15}/>{:else}<Copy size={15}/>{/if}</button><button onclick={()=>onretry(message)} title="Retry in a new branch" aria-label="Retry response in a new branch"><RotateCcw size={15}/></button><span class="outcome-label">{message.outcome==='complete'?'':message.outcome}</span>{#if copyError}<span>Clipboard unavailable</span>{/if}</div>{/if}
    </div>
  {/if}
</article>

<script>
  import { Copy, Check } from 'lucide-svelte';
  import {tokens,html,highlighted} from '../lib/markdown.js';
  export let text='';
  let copied=-1;
  async function copy(code,i) {try {await navigator.clipboard.writeText(code);copied=i;setTimeout(()=>copied=-1,1800);}catch {copied=-2;}}
</script>
<div class="prose">
  {#each tokens(text) as token,i}
    {#if token.type==='code'}
      <div class="code-block"><div class="code-toolbar"><span>{token.lang || 'Plain text'}</span><button onclick={()=>copy(token.text,i)} aria-label="Copy code">{#if copied===i}<Check size={13}/> Copied{:else}<Copy size={13}/> Copy code{/if}</button></div><pre><code>{@html highlighted(token.text,token.lang||'')}</code></pre></div>
    {:else}<div class="markdown-block">{@html html(token)}</div>{/if}
  {/each}
</div>

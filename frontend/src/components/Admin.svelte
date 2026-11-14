<script>
  import {onMount} from 'svelte';
  import {ArrowUp,Check,LoaderCircle,ShieldCheck,ChevronDown} from 'lucide-svelte';
  import {api} from '../lib/api.js';
  import {adminHistory} from '../lib/admin.js';
  let catalog=null,request='',entries=[],busy=false,error='',selected='',fields={},direct=false;
  $: tool=catalog?.tools.find(t=>t.name===selected);
  async function load(){catalog=await api('/control/catalog');}
  onMount(()=>{load().catch(e=>error=e.message);});
  async function propose(action){
    error='';busy=true;const label=action?`Direct action: ${action.tool}`:request.trim();if(!label){busy=false;return;}
    const history=adminHistory(entries);
    entries=[...entries,{kind:'request',text:label}];request='';
    try{const result=await api('/control/propose',{method:'POST',body:action?{action}:{request:label,history}});entries=[...entries,{kind:result.question?'question':'proposal',...result,text:result.question}];}
    catch(e){error=e.message;}finally{busy=false;}
  }
  async function execute(entry){busy=true;error='';try{const result=await api('/control/execute',{method:'POST',body:{id:entry.id}});entry.done=true;entries=[...entries,{kind:'result',...result}];await load();}catch(e){entry.expired=true;entries=[...entries];error=e.message;}finally{busy=false;}}
  function directAction(e){e.preventDefault();const args={};for(const [name,schema]of Object.entries(tool.input_schema.properties)){args[name]=schema.type==='boolean'?fields[name]==='true':fields[name]||'';}propose({tool:tool.name,arguments:args});}
</script>
<div class="admin-intro"><ShieldCheck size={22}/><div><h3>Manage through conversation</h3><p>Describe one change. Ordo prepares a precise action for you to review. Only your available capabilities are offered.</p></div></div>
<div class="admin-suggestions">{#each ['List accounts','Show today’s usage','Show feature flags','Show recent admin activity'] as example}<button class="secondary-button" disabled={busy} onclick={()=>request=example}>{example}</button>{/each}</div>
<div class="admin-conversation" aria-live="polite">
  {#each entries as entry}
    {#if entry.kind==='request'}<p class="admin-request">{entry.text}</p>
    {:else if entry.kind==='question'}<p class="admin-question">{entry.text}</p>
    {:else if entry.kind==='proposal'}<article class="admin-preview"><span class="eyebrow">{entry.review_required?'REVIEW PROPOSED CHANGE':'READY TO INSPECT'}</span><h4>{entry.action.tool}</h4><dl>{#each Object.entries(entry.action.arguments) as [key,value]}<dt>{key}</dt><dd>{String(value)}</dd>{/each}</dl><p>{entry.review_required?'No changes have been made. Confirm only if the action and target match your request.':'This query reads administrative metadata.'}</p><button class="primary-button" disabled={busy||entry.done||entry.expired} onclick={()=>execute(entry)}>{#if entry.done}<Check size={15}/> Completed{:else if entry.expired}Create a fresh preview{:else}{entry.review_required?'Confirm this change':'Run query'}{/if}</button></article>
    {:else if entry.kind==='result'}<article class="admin-result"><strong>{entry.already_executed?'Already completed':'Completed'}</strong>{#if entry.result?.invitation_url}<p>This link appears once. Share it privately with the intended student.</p><input aria-label="Private invitation link" readonly value={entry.result.invitation_url}/><button class="secondary-button" onclick={()=>navigator.clipboard.writeText(entry.result.invitation_url)}>Copy invitation</button>{:else}<pre>{JSON.stringify(entry.result||entry,null,2)}</pre>{/if}<small>Receipt: {entry.receipt||'Existing receipt'}</small></article>{/if}
  {/each}
</div>
{#if error}<p class="auth-error" role="alert">{error}</p>{/if}
<form class="admin-composer" onsubmit={e=>{e.preventDefault();propose();}}><label class="sr-only" for="admin-request">Administrative request</label><textarea id="admin-request" bind:value={request} maxlength="2000" placeholder="For example: invite a student with username alex373" rows="2" disabled={busy}></textarea><button class="send-button" aria-label="Prepare action" disabled={busy||!request.trim()}>{#if busy}<LoaderCircle class="spin" size={18}/>{:else}<ArrowUp size={18}/>{/if}</button></form>
<button class="admin-direct-toggle" onclick={()=>direct=!direct}>Choose an action directly <ChevronDown size={14}/></button>
{#if direct&&catalog}<form class="admin-direct" onsubmit={directAction}><label for="tool-select">Available actions</label><select id="tool-select" bind:value={selected} onchange={()=>fields={}}><option value="">Choose an action</option>{#each catalog.tools as t}<option value={t.name}>{t.name}</option>{/each}</select>{#if tool}<p>{tool.description}</p>{#each Object.entries(tool.input_schema.properties) as [name,schema]}<label for={'field-'+name}>{name}</label>{#if schema.type==='boolean'}<select id={'field-'+name} bind:value={fields[name]} required><option value="">Choose…</option><option value="true">Enabled / allow</option><option value="false">Disabled / deny</option></select>{:else if schema.enum}<select id={'field-'+name} bind:value={fields[name]} required><option value="">Choose…</option>{#each schema.enum as v}<option value={v}>{v}</option>{/each}</select>{:else}<input id={'field-'+name} bind:value={fields[name]} required maxlength="40"/>{/if}{/each}<button class="secondary-button" disabled={busy}>Prepare preview</button>{/if}</form>{/if}
<p class="admin-footnote">Previews expire after 10 minutes or when account policy changes. Commands and read access produce audit receipts. Invitation links and passwords never enter the planner.</p>

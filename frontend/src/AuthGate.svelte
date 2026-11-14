<script>
  import {onMount} from 'svelte';
  import {ArrowRight,ShieldCheck,LoaderCircle,Eye,EyeOff} from 'lucide-svelte';
  import App from './App.svelte';
  import {api} from './lib/api.js';
  let state=null,error='',busy=false,username='',password='',confirm='',visible=false;
  let invitation=new URLSearchParams(location.hash.slice(1)).get('invite')||'';
  if(invitation)history.replaceState(null,'',location.pathname+location.search);
  let channel;
  async function refresh(){try{state=await api('/auth/session');error='';}catch(e){error=e.message;}}
  function expired(){state={required:true,user:null};password='';confirm='';error='Your session ended. Sign in to continue.';}
  onMount(()=>{
    refresh();
    window.addEventListener('ordo:unauthorized',expired);
    channel=typeof BroadcastChannel!=='undefined'?new BroadcastChannel('ordo-account'):null;
    if(channel)channel.onmessage=()=>{state=null;refresh();};
    const focus=()=>refresh();window.addEventListener('focus',focus);
    return()=>{window.removeEventListener('ordo:unauthorized',expired);window.removeEventListener('focus',focus);channel?.close();};
  });
  async function submit(e){
    e.preventDefault();error='';if(invitation&&password!==confirm){error='Passwords do not match.';return;}busy=true;
    try{await api(invitation?'/auth/enroll':'/auth/login',{method:'POST',body:invitation?{token:invitation,password}:{username,password}});password='';confirm='';invitation='';await refresh();channel?.postMessage('changed');}
    catch(e){error=e.message;}finally{busy=false;}
  }
  async function logout(){try{await api('/auth/logout',{method:'POST'});state={required:true,user:null};history.replaceState(null,'',location.pathname);channel?.postMessage('changed');}catch(e){error=e.message;}}
</script>
{#if state&&(!state.required||state.user)}
  {#key state.user?.id||'local'}<App account={state.user} capabilities={state.capabilities||[]} onlogout={logout}/>{/key}
{:else}
  <main class="auth-page">
    <a class="auth-brand" href="/" aria-label="Ordo home"><span>✳</span> ordo<span class="auth-edition">IS373 WORKSPACE</span></a>
    <section class="auth-card" aria-labelledby="auth-title">
      <div class="auth-emblem">✳</div><p class="auth-eyebrow">A LITTLE SPACE TO THINK BIG</p>
      <h1 id="auth-title">{!state?'Opening your workspace…':invitation?'Make yourself at home.':'Welcome back.'}</h1>
      <p class="auth-intro">{invitation?'Your invitation is ready. Choose a password to open your private workspace.':'A thoughtful place to explore, create, and learn together.'}</p>
      {#if state}
        <form onsubmit={submit}>
          {#if !invitation}<label for="username">Username</label><input id="username" name="username" bind:value={username} autocomplete="username" autocapitalize="none" spellcheck="false" required minlength="3" maxlength="40" placeholder="Your username"/>{/if}
          <label for="password">{invitation?'Create a password':'Password'}</label><div class="password-field"><input id="password" name="password" bind:value={password} type={visible?'text':'password'} autocomplete={invitation?'new-password':'current-password'} required minlength="12" maxlength="128" placeholder={invitation?'At least 12 characters':'Your password'}/><button type="button" aria-label={visible?'Hide password':'Show password'} onclick={()=>visible=!visible}>{#if visible}<EyeOff size={18}/>{:else}<Eye size={18}/>{/if}</button></div>
          {#if invitation}<label for="confirm">Confirm password</label><input id="confirm" name="confirm" bind:value={confirm} type="password" autocomplete="new-password" required minlength="12" maxlength="128"/>{/if}
          {#if error}<p class="auth-error" role="alert">{error}</p>{/if}
          <button class="auth-submit" disabled={busy}>{#if busy}<LoaderCircle size={18} class="spin"/>{:else}{invitation?'Create my account':'Continue'}<ArrowRight size={18}/>{/if}</button>
        </form>
        <p class="auth-help">{invitation?'Invitation links are private and expire after 24 hours.':'Access is by invitation. For an invitation or password reset, contact your instructor.'}</p>
      {:else}{#if error}<p class="auth-error" role="alert">{error}</p><button class="auth-submit" onclick={refresh}>Try again</button>{:else}<LoaderCircle class="spin" size={22}/>{/if}{/if}
      <div class="auth-trust"><ShieldCheck size={16}/><span>Your conversations belong to your account.</span></div>
    </section>
    <footer class="auth-footer">Built for curiosity. Designed for focus.<span>Ordo · IS373</span></footer>
  </main>
{/if}

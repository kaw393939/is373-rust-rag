<script>
  import {onMount,tick} from 'svelte';
  import {PanelLeft,SquarePen,Search,Plus,ArrowUp,Square,ChevronDown,ChevronRight,BookOpen,Sparkles,PenLine,Compass,Code2,Paperclip,Mic,Sun,Moon,Monitor,Settings2,X,Download,Pin,Archive,RotateCcw,MoreHorizontal,Check,ArrowDown,FileText,ExternalLink,ShieldCheck,LoaderCircle,CornerDownLeft,GitBranch,AudioLines,MessageSquare,HardDrive} from 'lucide-svelte';
  import {consumeEvents} from './stream.js';
  import {api,exportConversation,mediaUrl} from './lib/api.js';
  import {mediaHint} from './lib/media.js';
  import Message from './components/Message.svelte';
  import Attachment from './components/Attachment.svelte';
  import Modal from './components/Modal.svelte';
  import Admin from './components/Admin.svelte';
  export let account=null,capabilities=[],onlogout=()=>{};
  let status=null,library=[],chats=[],conversation=null,draft='',attachments=[],mode='chat',retrieval='vector';
  let sidebar=window.innerWidth>=850,modal='',query='',rename='',preview=null,evidence=null,showArchived=false,modelMenu=false,chatMenu=false;
  let active=false,runningId=null,runConversation=null,controller,error='',toast='',uploading=false,dragging=false;
  function initialTheme(){try{return localStorage.getItem('ordo-theme')||'light';}catch{return 'light';}}
  let theme=initialTheme(),systemDark=window.matchMedia('(prefers-color-scheme: dark)').matches,textarea,fileInput,scrollArea,nearBottom=true,editing=null,recorder=null,recording=false,recordSeconds=0,recordTimer;
  const starters=[{icon:PenLine,title:'Create something',text:'Find the words for your next idea.',prompt:'Help me write a clear, engaging introduction for my project.',mode:'chat'},
    {icon:Compass,title:'Explore an idea',text:'Go from a question to a new perspective.',prompt:'Explain retrieval-augmented generation with a simple analogy and an example.',mode:'chat'},
    {icon:BookOpen,title:'Ask your sources',text:'Get answers grounded in your collection.',prompt:'Who organizes workshops at Harbor Club, and can visitors attend?',mode:'vector'},
    {icon:Code2,title:'Build with clarity',text:'Work through code, one good step at a time.',prompt:'Show me a small Rust function that counts words, with a test and a brief explanation.',mode:'chat'}];
  $: filteredChats=chats.filter(c=>c.archived===showArchived&&(!query||(c.title+' '+(c.preview||'')).toLowerCase().includes(query.toLowerCase())));
  $: pinnedChats=filteredChats.filter(c=>c.pinned);
  $: recentChats=filteredChats.filter(c=>!c.pinned);
  $: isCurrentRunning=active&&conversation?.id===runningId;
  $: messages=conversation?.messages||[];
  $: themeClass=theme==='system'?(systemDark?'dark':'light'):theme;
  function notify(text){toast=text;setTimeout(()=>toast='',2600);}
  async function refreshChats(){chats=await api('/conversations');}
  async function refreshStatus(){try{status=await api('/status');library=await api('/sources');}catch(e){error=e.message;}}
  onMount(()=>{
    sidebar=window.innerWidth>=850;
    const appearance=window.matchMedia('(prefers-color-scheme: dark)');systemDark=appearance.matches;const appearanceChanged=e=>systemDark=e.matches;appearance.addEventListener('change',appearanceChanged);
    try{theme=localStorage.getItem('ordo-theme')||'light';}catch{}
    Promise.all([refreshStatus(),refreshChats()]).then(()=>{const id=location.hash.replace('#chat=','');if(location.hash.startsWith('#chat=')&&chats.some(c=>c.id===id))chooseChat(id);}).catch(e=>error=e.message);
    const keys=e=>{if(e.key==='Escape'){modelMenu=false;chatMenu=false;}if((e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==='k'){e.preventDefault();modal='search';query='';}if((e.metaKey||e.ctrlKey)&&e.shiftKey&&e.key.toLowerCase()==='o'){e.preventDefault();newChat();}};
    window.addEventListener('keydown',keys);
    return()=>{appearance.removeEventListener('change',appearanceChanged);window.removeEventListener('keydown',keys);controller?.abort();recorder?.stream?.getTracks().forEach(t=>t.stop());clearInterval(recordTimer);};
  });
  function setTheme(value){theme=value;try{localStorage.setItem('ordo-theme',value);}catch{}}
  async function chooseChat(id){
    try{conversation=id===runningId&&active?runConversation:await api(`/conversations/${id}`);history.replaceState(null,'',`#chat=${id}`);const last=conversation.messages.at(-1);mode=(last?.mode==='vector'||last?.mode==='keyword'||last?.evidence?.length)?'vector':'chat';if(last?.mode==='keyword')retrieval='keyword';draft='';attachments=[];editing=null;evidence=null;error='';modal='';chatMenu=false;query='';if(window.innerWidth<850)sidebar=false;await tick();bottom();}catch(e){error=e.message;}
  }
  function newChat(){history.replaceState(null,'',location.pathname);conversation=null;draft='';attachments=[];editing=null;evidence=null;error='';chatMenu=false;modal='';if(window.innerWidth<850)sidebar=false;tick().then(()=>textarea?.focus());}
  function bottom(){if(scrollArea)scrollArea.scrollTop=scrollArea.scrollHeight;nearBottom=true;}
  function resize(){if(textarea){textarea.style.height='auto';textarea.style.height=Math.min(textarea.scrollHeight,190)+'px';}}
  async function selectStarter(starter){mode=starter.mode;draft=starter.prompt;await tick();resize();textarea?.focus();}
  async function updateChat(patch){try{await api(`/conversations/${conversation.id}`,{method:'PATCH',body:patch});conversation={...conversation,...patch};await refreshChats();chatMenu=false;}catch(e){error=e.message;}}
  async function archiveChat(){await updateChat({archived:!conversation.archived});notify(conversation.archived?'Conversation archived. Find it in Archived.':'Conversation restored.');if(conversation.archived)newChat();}
  async function branch(message,retry=false){
    if(active){notify('Stop the current response before creating a branch.');return;}
    try{
      let original=message;
      if(message.role==='assistant'){const index=messages.findIndex(m=>m.id===message.id);original=[...messages.slice(0,index)].reverse().find(m=>m.role==='user');}
      if(!original)return;
      conversation=await api(`/conversations/${conversation.id}/fork`,{method:'POST',body:{before:original.id}});
      history.replaceState(null,'',`#chat=${conversation.id}`);mode=original.mode==='vector'||original.mode==='keyword'?'vector':'chat';if(original.mode==='keyword')retrieval='keyword';draft=original.content;attachments=original.attachments||[];editing=retry?null:'Editing in a new branch. Your original conversation is preserved.';
      await refreshChats();await tick();resize();textarea?.focus();if(retry)await send();
    }catch(e){error=e.message;}
  }
  async function send(){
    if(active||uploading||recording||(!draft.trim()&&!attachments.length))return;
    error='';editing=null;active=true;
    const content=draft.trim()||'Please help me with the attached files.';
    const files=[...attachments];const selectedMode=mode==='vector'?retrieval:mode;
    try{
      if(!conversation)conversation=await api('/conversations',{method:'POST'});
      history.replaceState(null,'',`#chat=${conversation.id}`);
      const c={...conversation,messages:[...conversation.messages]};
      const user={id:crypto.randomUUID(),role:'user',mode:selectedMode,content,attachments:files,evidence:[],outcome:'complete',created:Date.now()};
      const assistant={id:crypto.randomUUID(),role:'assistant',mode:selectedMode,content:'',attachments:[],evidence:[],outcome:'streaming',created:Date.now(),model:status?.chat_model};
      c.messages.push(user,assistant);if(c.title==='New conversation')c.title=content.slice(0,65);
      conversation=c;runConversation=c;chats=[{...c,message_count:c.messages.length,preview:content},...chats.filter(item=>item.id!==c.id)];active=true;runningId=c.id;controller=new AbortController();draft='';attachments=[];await tick();resize();bottom();
      const response=await fetch('/api/chat',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({question:content,mode:selectedMode,conversation_id:c.id,attachments:files.map(f=>f.id)}),signal:controller.signal});
      if(response.status===401)window.dispatchEvent(new Event('ordo:unauthorized'));
      if(!response.ok){let data;try{data=await response.json();}catch{}if(conversation?.id===c.id){conversation=await api(`/conversations/${c.id}`);draft=content;attachments=files;}runConversation=null;throw new Error(data?.error||'Unable to start this response.');}
      await consumeEvents(response.body,(name,data)=>{
        if(name==='run.started'){assistant.id=data.message_id;user.id=data.user_message_id;assistant.model=data.model;}
        if(name==='input.progress'){assistant.stage=data.message;if(data.model)assistant.model=data.model;if(data.attachment_id)user.attachments=user.attachments.map(f=>f.id===data.attachment_id?{...f,input_state:'processing'}:f);}
        if(name==='input.ready')user.attachments=data.attachments;
        if(name==='retrieval.evidence_selected')assistant.evidence=data.sources;
        if(name==='answer.delta')assistant.content+=data.text;
        if(name==='run.ended'){assistant.outcome=data.outcome;assistant.notice=data.message;}
        runConversation={...c,messages:[...c.messages]};if(conversation?.id===c.id)conversation=runConversation;
        if(nearBottom)tick().then(()=>bottom());
      });
    }catch(e){
      if(runConversation){for(const item of runConversation.messages){item.attachments=(item.attachments||[]).map(f=>f.input_state==='processing'?{...f,input_state:''}:f);}let m=runConversation.messages.at(-1);if(m?.role==='assistant'){m.outcome=e.name==='AbortError'?'cancelled':'failed';m.notice=e.name==='AbortError'?'Stopped. You can retry in a new branch.':e.message;runConversation={...runConversation};if(conversation?.id===runningId)conversation=runConversation;}}
      else error=e.message;
    }finally{active=false;await refreshChats().catch(()=>{});runningId=null;runConversation=null;}
  }
  async function uploadFiles(files){
    if(uploading)return;error='';uploading=true;
    try{
      if(attachments.length+files.length>4)throw new Error('You can attach up to four files per message.');
      for(const file of files){if(file.size>12*1024*1024)throw new Error(`${file.name} exceeds 12 MB.`);const form=new FormData();form.append('file',file);const result=await api('/attachments',{method:'POST',body:form});attachments=[...attachments,result];}
    }catch(e){error=e.message;}finally{uploading=false;if(fileInput)fileInput.value='';}
  }
  async function startRecording(){
    if(recording){recorder.stop();return;}
    if(!navigator.mediaDevices||!window.MediaRecorder){error='Audio recording is not supported by this browser. You can attach an audio file instead.';return;}
    let stream;
    try{
      stream=await navigator.mediaDevices.getUserMedia({audio:true});let parts=[];
      recorder=new MediaRecorder(stream);recorder.ondataavailable=e=>{if(e.data.size)parts.push(e.data);};
      recorder.onstop=()=>{clearInterval(recordTimer);stream.getTracks().forEach(t=>t.stop());recording=false;const type=recorder.mimeType;uploadFiles([new File(parts,`Voice note ${new Date().toLocaleTimeString().replaceAll(':','-')}.${type.includes('mp4')?'m4a':'weba'}`,{type})]);};
      recording=true;recordSeconds=0;recorder.start();recordTimer=setInterval(()=>{recordSeconds++;if(recordSeconds>=119)recorder.stop();},1000);
    }catch(e){stream?.getTracks().forEach(t=>t.stop());error='Microphone access was unavailable. Check browser permissions or attach an audio file.';}
  }
  function onDrop(e){e.preventDefault();dragging=false;if(e.dataTransfer.files.length)uploadFiles([...e.dataTransfer.files]);}
  function onPaste(e){const files=[...e.clipboardData.files];if(files.length){e.preventDefault();uploadFiles(files);}}
</script>

<svelte:window onclick={e=>{if(!e.target.closest?.('.model-control,.chat-menu-wrap')){modelMenu=false;chatMenu=false;}}}/>

<svelte:head><title>{conversation?conversation.title+' · Ordo':'Ordo — A space for better thinking'}</title><meta name="description" content="A beautifully focused, local conversation workspace. Chat, explore your sources, and bring your files into the conversation."/></svelte:head>

<div class="app" data-theme={themeClass}>
  {#if sidebar}<button class="mobile-scrim" aria-label="Close navigation" onclick={()=>sidebar=false}></button>{/if}
  <aside class:collapsed={!sidebar} class="sidebar" inert={!sidebar} aria-hidden={!sidebar}>
    <div class="sidebar-brand"><button class="wordmark" onclick={newChat} aria-label="Ordo home"><span class="brand-mark">✳</span> ordo<span class="brand-period">.</span></button><button class="icon-button sidebar-toggle" aria-label="Collapse sidebar" onclick={()=>sidebar=false}><PanelLeft size={18}/></button></div>
    <button class="new-chat" onclick={newChat}><SquarePen size={16}/> New conversation <span>⌘ ⇧ O</span></button>
    <button class="nav-item" onclick={()=>{modal='search';query='';}}><Search size={17}/> Search conversations <kbd>⌘ K</kbd></button>
    <button class="nav-item" onclick={()=>modal='library'}><BookOpen size={17}/> Knowledge library <span class="tiny-count">{library.length}</span></button>
    <div class="sidebar-divider"></div>
    <div class="history-heading"><span>{showArchived?'ARCHIVED':'YOUR CONVERSATIONS'}</span><button class="icon-button" class:active={showArchived} onclick={()=>showArchived=!showArchived} title={showArchived?'Show recent conversations':'Show archived conversations'} aria-label={showArchived?'Show recent conversations':'Show archived conversations'}><Archive size={14}/></button></div>
    <nav class="chat-history" aria-label="Conversation history">
      {#if pinnedChats.length}<div class="history-label">Pinned</div>{#each pinnedChats as chat}<button class:selected={conversation?.id===chat.id} class="chat-item" onclick={()=>chooseChat(chat.id)}><Pin size={12}/><span>{chat.title}</span>{#if active&&runningId===chat.id}<i class="running-dot"></i>{/if}</button>{/each}<div class="history-label">Recent</div>{/if}
      {#each recentChats as chat}<button class:selected={conversation?.id===chat.id} class="chat-item" onclick={()=>chooseChat(chat.id)}><MessageSquare size={14}/><span>{chat.title}</span>{#if active&&runningId===chat.id}<i class="running-dot"></i>{/if}</button>{/each}
      {#if !filteredChats.length}<div class="history-empty"><MessageSquare size={20}/><p>{showArchived?'No archived conversations.':'A fresh space for your ideas.'}</p><small>{showArchived?'Archived chats will appear here.':'Your conversations will be saved here.'}</small></div>{/if}
    </nav>
    <div class="sidebar-bottom"><div class="privacy-note"><ShieldCheck size={15}/><div><strong>{status?.local_only?'Your space. Your machine.':'Your conversation workspace.'}</strong><span>{status?.local_only?'Conversations stay local.':'Check processing providers in settings.'}</span></div></div><button class="profile-button" onclick={()=>modal='settings'}><span class="profile-avatar">{account?.username.slice(0,2).toUpperCase()||'OR'}</span><span><strong>{account?.username||'Personal workspace'}</strong><small>{status?.local_only?'Local edition':'Configured workspace'}</small></span><Settings2 size={16}/></button></div>
  </aside>

  <main class="main-shell">
    <header class="topbar"><div class="topbar-left">{#if !sidebar}<button class="icon-button" aria-label="Open sidebar" onclick={()=>sidebar=true}><PanelLeft size={20}/></button>{/if}<div class="model-control"><button class="model-button" onclick={()=>{modelMenu=!modelMenu;chatMenu=false;}} aria-expanded={modelMenu}>Ordo <span>{status?.chat_model?.replace(':latest','')||'Connecting'}</span><ChevronDown size={14}/></button>{#if modelMenu}<div class="popover model-popover"><div class="popover-label">ACTIVE MODEL</div><div class="model-option"><span class="model-symbol">✳</span><div><strong>{status?.chat_model||'Connecting…'}</strong><small>{status?.chat_provider||'Connecting'} · {status?.local_only?'local inference':'configured inference'}</small></div><Check size={16}/></div><p>{status?.capabilities?.image_input?'Images → '+status.vision_model:'Image understanding unavailable'}<br/>{status?.capabilities?.audio_input?'Audio → '+(status.capabilities.audio_local?'local ':'')+'speech transcription':'Audio transcription unavailable'}</p><button class="popover-row" onclick={()=>{modal='settings';modelMenu=false;}}><Settings2 size={15}/> Workspace settings <ChevronRight size={14}/></button></div>{/if}</div></div>
      <div class="topbar-right"><span class="private-label"><span class="status-dot"></span> {status?.local_only?'Private workspace':'Personal workspace'}</span>{#if conversation}<button class="icon-button" title="Export conversation" aria-label="Export conversation" onclick={()=>exportConversation(conversation)}><Download size={17}/></button><div class="chat-menu-wrap"><button class="icon-button" aria-label="Conversation options" aria-expanded={chatMenu} onclick={()=>{chatMenu=!chatMenu;modelMenu=false;}}><MoreHorizontal size={20}/></button>{#if chatMenu}<div class="popover chat-popover"><button class="popover-row" onclick={()=>{rename=conversation.title;modal='rename';chatMenu=false;}}><PenLine size={15}/> Rename</button><button class="popover-row" onclick={()=>updateChat({pinned:!conversation.pinned})}><Pin size={15}/>{conversation.pinned?'Unpin':'Pin conversation'}</button><button class="popover-row" onclick={archiveChat}><Archive size={15}/>{conversation.archived?'Restore conversation':'Archive conversation'}</button></div>{/if}</div>{/if}</div>
    </header>
    {#if active&&!isCurrentRunning}<div class="background-run"><LoaderCircle size={14} class="spin"/> A response is running in another conversation.<button onclick={()=>chooseChat(runningId)}>Return to chat</button><button onclick={()=>controller?.abort()}>Stop</button></div>{/if}
    <div class="content-layout">
      <div class="conversation-pane">
        <div class="conversation-scroll" bind:this={scrollArea} onscroll={()=>nearBottom=scrollArea.scrollHeight-scrollArea.scrollTop-scrollArea.clientHeight<100}>
          {#if messages.length}
            <div class="conversation-title"><span>{conversation.title}</span><span>{new Date(conversation.created).toLocaleDateString(undefined,{month:'short',day:'numeric'})}</span></div>
            <div class="messages">{#each messages as message (message.id)}<Message {message} capabilities={status?.capabilities||{}} active={isCurrentRunning&&message.outcome==='streaming'} onretry={m=>branch(m,true)} onedit={m=>branch(m)} onsources={m=>evidence=m} onpreview={f=>preview=f}/>{/each}</div>
          {:else}
            <div class="welcome"><div class="welcome-emblem"><span>✳</span></div><div class="welcome-eyebrow">A SPACE FOR BETTER THINKING</div><h1>A little curiosity.<br/><em>A lot of possibility.</em></h1><p>Bring an idea, a question, or a work in progress.<br class="desktop-break"/> Let’s make something of it.</p></div>
            <div class="starter-grid">{#each starters as starter}<button class="starter-card" onclick={()=>selectStarter(starter)}><svelte:component this={starter.icon} size={20} strokeWidth={1.5}/><strong>{starter.title}</strong><span>{starter.text}</span></button>{/each}</div>
          {/if}
        </div>
        <div class="composer-zone">
          {#if !nearBottom&&messages.length}<button class="jump-bottom" aria-label="Scroll to latest message" onclick={bottom}><ArrowDown size={17}/></button>{/if}
          {#if conversation?.archived}<div class="archived-banner"><Archive size={16}/> This conversation is archived.<button onclick={()=>updateChat({archived:false})}>Restore to continue</button></div>{:else}
            {#if editing}<div class="edit-banner"><GitBranch size={14}/>{editing}<button class="icon-button" aria-label="Dismiss branch notice" onclick={()=>editing=null}><X size={13}/></button></div>{/if}
            {#if error}<div class="composer-error" role="alert"><span>{error}</span><button class="icon-button" aria-label="Dismiss error" onclick={()=>error=''}><X size={14}/></button></div>{/if}
            <!-- svelte-ignore a11y_no_static_element_interactions (drop zone supplements the accessible file picker) -->
            <div class:dragging class:responding={isCurrentRunning} class="composer" ondragover={e=>{e.preventDefault();dragging=true;}} ondragleave={e=>{if(!e.currentTarget.contains(e.relatedTarget))dragging=false;}} ondrop={onDrop}>
              {#if dragging}<div class="drop-overlay"><Paperclip size={25}/><strong>Drop files into the conversation</strong><span>Up to 4 files · 12 MB each</span></div>{/if}
              {#if attachments.length}<div class="composer-attachments">{#each attachments as file}<Attachment {file} capabilities={status?.capabilities||{}} compact onremove={id=>attachments=attachments.filter(f=>f.id!==id)} onpreview={f=>preview=f}/>{/each}</div>{/if}
              <label class="sr-only" for="composer">Message Ordo</label><textarea bind:this={textarea} id="composer" bind:value={draft} oninput={resize} onpaste={onPaste} onkeydown={e=>{if(e.key==='Enter'&&!e.shiftKey&&!e.isComposing){e.preventDefault();send();}}} placeholder={mode==='vector'?'Ask a question about your sources…':'Where shall we start?'} rows="1" maxlength="12000"></textarea>
              <div class="composer-toolbar"><div class="composer-tools"><input bind:this={fileInput} type="file" class="sr-only" multiple accept="image/png,image/jpeg,image/webp,image/gif,audio/*,video/mp4,video/webm,.pdf,.txt,.md,.csv,.json" onchange={e=>uploadFiles([...e.target.files])}/><button class="icon-button attach-button" onclick={()=>fileInput.click()} disabled={uploading||attachments.length>=4} aria-label="Attach files" title="Attach images, audio, video, PDF or text">{#if uploading}<LoaderCircle class="spin" size={20}/>{:else}<Plus size={22} strokeWidth={1.6}/>{/if}</button><span class="tool-divider"></span><div class="mode-switch" aria-label="Conversation mode"><button class:chosen={mode==='chat'} onclick={()=>mode='chat'} title="General conversation"><Sparkles size={14}/> Chat</button><button class:chosen={mode==='vector'} onclick={()=>mode='vector'} title="Answer from your indexed sources"><BookOpen size={14}/> Research</button></div></div><div class="send-tools">{#if recording}<span class="record-timer"><i></i>{Math.floor(recordSeconds/60)}:{String(recordSeconds%60).padStart(2,'0')}</span>{/if}<button class:recording class="icon-button mic-button" aria-label={recording?'Finish audio recording':'Record audio attachment'} title={recording?'Finish recording':'Record a voice note · transcribed on send'} onclick={startRecording} disabled={uploading||attachments.length>=4}>{#if recording}<Square size={15} fill="currentColor"/>{:else}<Mic size={19}/>{/if}</button>{#if isCurrentRunning}<button class="send-button stop-button" aria-label="Stop response" onclick={()=>controller?.abort()}><Square size={14} fill="currentColor"/></button>{:else}<button class="send-button" aria-label="Send message" onclick={send} disabled={active||uploading||recording||(!draft.trim()&&!attachments.length)}><ArrowUp size={20}/></button>{/if}</div></div>
            </div>
            <div class="composer-footnote"><span>{attachments.length?mediaHint(attachments,status?.capabilities):mode==='vector'?'Grounded in your library. Always check the sources.':'A little human judgment is always a good idea.'}</span><span class="keyboard-hint">⇧ Enter for a new line</span></div>
          {/if}
        </div>
      </div>
      {#if evidence}<aside class="evidence-drawer"><div class="drawer-heading"><div><span class="eyebrow">BEHIND THE ANSWER</span><h2>Sources & context</h2></div><button class="icon-button" aria-label="Close sources" onclick={()=>evidence=null}><X size={19}/></button></div><div class="evidence-summary"><BookOpen size={16}/>{evidence.evidence.length} evidence items<span>Sources include retrieved text and supplied media.</span></div><div class="evidence-list">{#each evidence.evidence as source,i}<article class="source-card"><div class="source-top"><span class="source-number">{i+1}</span><code>{source.id}</code><span>{source.score.toFixed(2)}</span></div><h3>{source.title}</h3><p>{source.text}</p><a href={source.origin} target="_blank" rel="noreferrer">Open original <ExternalLink size={12}/></a></article>{/each}</div></aside>{/if}
    </div>
  </main>
  {#if toast}<div class="toast" role="status"><Check size={16}/>{toast}</div>{/if}

  {#if modal==='search'}<Modal title="Find a conversation" onclose={()=>{modal='';query='';}}><div class="search-field"><Search size={19}/><input aria-label="Search conversations" bind:value={query} placeholder="Search titles and recent prompts…"/></div><div class="search-results">{#each filteredChats as chat}<button onclick={()=>chooseChat(chat.id)}><MessageSquare size={17}/><span><strong>{chat.title}</strong><small>{chat.preview||'Empty conversation'}</small></span><CornerDownLeft size={14}/></button>{/each}{#if !filteredChats.length}<p class="empty-search">No conversations found. Try a different phrase.</p>{/if}</div><div class="modal-footer-note">Searching {showArchived?'archived':'active'} conversations · Esc to close</div></Modal>{/if}
  {#if modal==='rename'}<Modal title="Rename conversation" onclose={()=>modal=''}><form class="rename-form" onsubmit={async e=>{e.preventDefault();await updateChat({title:rename});modal='';}}><label for="rename">Conversation title</label><input id="rename" bind:value={rename} maxlength="100" required/><div class="modal-actions"><button class="secondary-button" type="button" onclick={()=>modal=''}>Cancel</button><button class="primary-button" disabled={!rename.trim()}>Save name</button></div></form></Modal>{/if}
  {#if modal==='library'}<Modal title="Your knowledge library" wide onclose={()=>modal=''}><div class="library-intro"><span class="library-icon"><BookOpen size={26}/></span><div><h3>Harbor Club collection</h3><p>{library.length} documents · {status?.index?.chunk_count||0} indexed passages<br/>A fictional collection for exploring source-grounded answers.</p></div></div><div class="library-grid">{#each library as source}<a href={source.origin} target="_blank" rel="noreferrer"><FileText size={22}/><strong>{source.title.replace('Fictional Harbor Club ','')}</strong><span>Teaching document <ExternalLink size={12}/></span></a>{/each}</div><div class="library-bottom"><ShieldCheck size={16}/> Indexed locally. Original sources remain inspectable.<button onclick={()=>{modal='';mode='vector';textarea?.focus();}}>Ask your library <ChevronRight size={15}/></button></div></Modal>{/if}
  {#if modal==='settings'}<Modal title="Make yourself at home" onclose={()=>modal=''}><div class="settings-section">{#if account}<h3>Your account</h3><p>Signed in as <strong>{account.username}</strong>. Conversations and files are private to this account.</p><div class="account-actions">{#if capabilities.some(c=>['user:read','user:invite','user:manage','policy:manage','usage:read','audit:read'].includes(c))}<button class="primary-button" onclick={()=>modal='admin'}><ShieldCheck size={15}/> Administration</button>{/if}<button class="secondary-button" onclick={onlogout}>Sign out</button></div>{/if}<h3>Appearance</h3><p>A workspace that feels right, day or night.</p><div class="theme-options">{#each [{id:'light',label:'Light',icon:Sun},{id:'dark',label:'Dark',icon:Moon},{id:'system',label:'System',icon:Monitor}] as option}<button class:selected={theme===option.id} onclick={()=>setTheme(option.id)}><svelte:component this={option.icon} size={22}/><span>{option.label}</span>{#if theme===option.id}<Check size={13}/>{/if}</button>{/each}</div></div><div class="settings-section"><h3>Model & capabilities</h3><div class="settings-model"><span class="brand-mark">✳</span><div><strong>{status?.chat_model||'Connecting'}</strong><small>{status?.chat_provider} · {status?.chat_available?'Ready for conversation':'Unavailable'}</small></div><span class="local-badge">{status?.local_only?'LOCAL':'CONFIGURED'}</span></div><p class="setup-note">{status?.chat_setup}</p><div class="capability-list"><div><span class="capability-symbol"><Paperclip size={17}/></span><span><strong>Image understanding</strong><small>{status?.vision_model} · {status?.vision_provider}</small></span><span class:unavailable={!status?.vision_available} class="capability-state">{status?.vision_available?'Ready':'Unavailable'}</span></div><div><span class="capability-symbol"><AudioLines size={17}/></span><span><strong>Speech transcription</strong><small>{status?.transcription?.model||'Whisper'} · up to 2 minutes</small></span><span class:unavailable={!status?.transcription?.ready} class="capability-state">{status?.transcription?.ready?'Ready':'Needs setup'}</span></div><div><span class="capability-symbol"><FileText size={17}/></span><span><strong>PDF understanding</strong><small>Page text + rendered scans · up to 20 pages</small></span><span class:unavailable={!status?.capabilities?.pdf_input} class="capability-state">{status?.capabilities?.pdf_input?'Ready':'Needs setup'}</span></div><div><span class="capability-symbol"><Compass size={17}/></span><span><strong>Video understanding</strong><small>Sampled frames + speech · up to 2 minutes</small></span><span class:unavailable={!status?.capabilities?.video_input} class="capability-state">{status?.capabilities?.video_input?'Ready':'Needs setup'}</span></div></div><p>Images are sent to the vision model. Voice notes and audio files become saved transcripts the assistant can read. PDFs supply page text and rendered scans. Videos supply sampled frames and a speech transcript. Recognition can be imperfect; events between frames may be missed.</p>{#each [status?.document_processing?.message,status?.video_processing?.message].filter(Boolean) as message}<p class="setup-note">{message}</p>{/each}{#if status?.transcription?.message}<p class="setup-note">{status.transcription.message}</p>{/if}<button class="secondary-button" onclick={refreshStatus}><RotateCcw size={13}/> Refresh model status</button></div><div class="settings-section"><h3>Research retrieval</h3><p>Compare semantic similarity with a simple keyword baseline.</p><select class="settings-select" aria-label="Research retrieval method" bind:value={retrieval}><option value="vector">Semantic similarity</option><option value="keyword">Keyword baseline</option></select></div><div class="settings-section"><h3>Storage & privacy</h3><p>Conversations and files are saved on the server for this workspace. {status?.local_only?'Nothing is sent to a cloud model.':'Configured providers may receive message and attachment content for processing.'} Archiving hides a conversation without deleting it.</p><div class="settings-status"><HardDrive size={15}/> SQLite storage <span>{status?.ready?'Library indexed':'Library needs setup'}</span></div></div><div class="modal-footer-note">Ordo · Personal workspace</div></Modal>{/if}
  {#if modal==='admin'}<Modal title="Workspace administration" wide onclose={()=>modal=''}><Admin/></Modal>{/if}
  {#if preview}<Modal title={preview.name} wide onclose={()=>preview=null}><div class="lightbox"><img src={mediaUrl(preview)} alt={preview.name}/></div><div class="lightbox-footer"><span>Saved locally · {status?.capabilities?.image_input?'Available to the vision model':'Vision unavailable'}</span><a class="secondary-button" href={mediaUrl(preview)} download={preview.name}><Download size={15}/> Download</a></div></Modal>{/if}
</div>

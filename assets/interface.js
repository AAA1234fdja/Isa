const promptBox=document.querySelector('#prompt'),thread=document.querySelector('#thread');
document.querySelectorAll('[data-info]').forEach(button=>button.addEventListener('click',()=>isaNotifications.show(button.dataset.info)));document.querySelectorAll('[data-name], [data-hub]').forEach(button=>button.addEventListener('click',()=>showHub(button.dataset.name || button.dataset.hub)));document.querySelector('#more').onclick=()=>isaNotifications.show('This is a visual preview of ISA.');document.querySelector('#project').onclick=showProjects;const welcome=document.querySelector('#welcome');
const heatmap=document.querySelector('#heatmap');
const chatStorageKey='isa.chats.v1';
let chats=[];
let activeChatId=null;
// Custom provider metadata is local to this browser. API keys are tab-session only.
const customProviderKey='isa.customProviders.v1',customSecretPrefix='isa.provider.key.';let customProviders=[];
function validProviderEndpoint(value){return isaProviderUtils.validProviderEndpoint(value)}
try{const list=JSON.parse(localStorage.getItem(customProviderKey)||'[]');if(Array.isArray(list))customProviders=list.filter(p=>p&&typeof p.id==='string'&&/^custom-[a-z0-9-]{1,80}$/.test(p.id)&&typeof p.name==='string'&&p.name.length<=80&&typeof p.model==='string'&&p.model.length<=160&&typeof p.url==='string'&&validProviderEndpoint(p.url)).slice(0,50).map(p=>({id:p.id,name:p.name,model:p.model,url:p.url,systemPrompt:typeof p.systemPrompt==='string'?p.systemPrompt.slice(0,8000):'',strictPoolJson:p.strictPoolJson===true,outputTokenField:p.outputTokenField==='max_completion_tokens'?'max_completion_tokens':'max_tokens',reasoningBudgetSupported:p.reasoningBudgetSupported===true,streamPool:p.streamPool===true,streamChat:p.streamChat===true}))}catch(error){isaNotifications.show('Could not load custom providers.')}
function providerSecret(id){try{return sessionStorage.getItem(customSecretPrefix+id)||''}catch(error){return ''}}
// Strict, bounded subset of OpenAI-compatible SSE. Unfinished text is never a completed answer.
const providerClient=isaProviderClient.createProviderClient({getProviders:()=>customProviders,getSecret:providerSecret,fetch:(...args)=>fetch(...args),getDecoder:()=>TextDecoder,createAbortError:()=>new DOMException('Stopped','AbortError')});
function consumePoolStream(...args){return providerClient.consumeStream(...args)}
function requestCustomProvider(...args){return providerClient.request(...args)}
const pendingChats=new Map(),pendingPoolProgress=new Map(),pendingPoolDraft=new Map(),activeToolLoops=new Map();let routingConfirmationPending=false,voiceContext=0,stopVoiceOnNavigate=()=>{};
const defaultChatSettings=()=>({provider:'auto',pluginsMode:'auto',enabledPlugins:null,softwareEngineeringEnabled:true,agentLoop:false,toolLoopOnSend:false,poolSymbolQuery:'',poolSymbolMode:'declarations',approvalMode:'always'});let draftChatSettings=defaultChatSettings();
const inspector=document.querySelector('#inspector'),inspectorBody=document.querySelector('#inspectorBody'),inspectorTitle=document.querySelector('#inspectorTitle'),refreshChanges=document.querySelector('#refreshChanges'),taskOverviewToggle=document.querySelector('#taskOverviewToggle'),changesToggle=document.querySelector('#changesToggle');let inspectorMode=null,changesSequence=0;
const inspectorResize=document.querySelector('#inspectorResize'),workspaceBody=document.querySelector('.workspace-body');let inspectorDesired=370;
try{const stored=Number(localStorage.getItem('isa.inspector.width'));if(stored>=180&&stored<=1200)inspectorDesired=stored}catch(error){}



let resizeDrag=null;inspectorResize.addEventListener('pointerdown',event=>{if(event.button!==0||!event.isPrimary)return;event.preventDefault();const limits=inspectorLimits();resizeDrag={pointer:event.pointerId,startX:event.clientX,startWidth:inspector.getBoundingClientRect().width/limits.scale,original:inspectorDesired};inspectorResize.setPointerCapture(event.pointerId);inspectorResize.classList.add('dragging');document.body.classList.add('inspector-resizing')});
inspectorResize.addEventListener('pointermove',event=>{if(!resizeDrag||event.pointerId!==resizeDrag.pointer)return;const limits=inspectorLimits();inspectorDesired=Math.max(limits.min,Math.min(limits.max,resizeDrag.startWidth+(resizeDrag.startX-event.clientX)/limits.scale));applyInspectorWidth()});

inspectorResize.addEventListener('pointerup',event=>finishInspectorResize(event,false));inspectorResize.addEventListener('pointercancel',event=>finishInspectorResize(event,true));inspectorResize.addEventListener('lostpointercapture',event=>finishInspectorResize(event,true));
inspectorResize.addEventListener('keydown',event=>{const limits=inspectorLimits(),current=applyInspectorWidth(),step=event.shiftKey?40:10;let next;if(event.key==='ArrowLeft')next=current+step;else if(event.key==='ArrowRight')next=current-step;else if(event.key==='Home')next=limits.min;else if(event.key==='End')next=limits.max;else return;event.preventDefault();inspectorDesired=Math.max(limits.min,Math.min(limits.max,next));applyInspectorWidth();saveInspectorWidth()});
window.addEventListener('resize',applyInspectorWidth);if(window.ResizeObserver)new ResizeObserver(applyInspectorWidth).observe(workspaceBody);applyInspectorWidth();

let region='US';try{const savedRegion=localStorage.getItem('isa.region');if(savedRegion==='CO'||savedRegion==='US')region=savedRegion}catch(error){}
try { const saved=JSON.parse(localStorage.getItem(chatStorageKey)||'[]'); if(Array.isArray(saved)) chats=saved.filter(chat=>chat && typeof chat.id==='string' && typeof chat.title==='string' && Array.isArray(chat.messages)).map(chat=>({...chat,messages:chat.messages.filter(message=>message && typeof message.text==='string' && typeof message.at==='string' && !Number.isNaN(Date.parse(message.at)))})); } catch(error){isaNotifications.show('Could not load saved chats.');}

// Abstract vector characters; stable indices preserve existing conversation selections.
const chatAvatars=[
 ['Orbit','#dcebe3','#538774','#254e40','M32 10a22 22 0 1 0 0 44a22 22 0 1 0 0-44Z',0],
 ['Peak','#f4e7d0','#d19358','#634731','M32 8L56 51Q57 54 52 54H12Q7 54 9 50Z',1],
 ['Cloud','#e0eaf0','#7297ab','#334f5c','M14 24C9 15 20 9 26 15C31 6 42 9 43 17C53 15 58 23 53 30C57 40 49 48 43 47C37 55 27 55 22 48C12 49 8 41 12 34C7 30 9 25 14 24Z',2],
 ['Bloom','#f1dfdf','#c97f7b','#613e45','M32 13C39 4 46 12 45 19C56 17 57 27 50 32C57 40 51 47 44 44C42 56 33 57 29 49C21 56 14 49 18 42C7 40 8 31 16 28C10 20 17 14 24 18C24 9 29 8 32 13Z',3],
 ['Ripple','#dbece9','#4f9693','#275658','M13 14C22 8 28 18 35 14S49 9 53 15V49C45 55 39 45 32 50S18 55 11 49V18Z',4],
 ['Gem','#e9e2f2','#967ab3','#48365b','M32 7L53 20L50 44L32 57L14 44L11 20Z',5],
 ['Comet','#f7e6d5','#d69769','#62462f','M11 17Q31 4 49 17Q58 24 52 35Q49 47 31 52Q11 52 9 37Q7 26 11 17Z',6],
 ['Leaf','#e4eddb','#8da76e','#405836','M12 38C11 19 30 8 52 10C54 31 44 52 25 53C17 52 12 46 12 38Z',7],
 ['Moon','#e1e7f2','#8296bb','#3a4b6b','M36 9C22 12 18 24 21 35C24 45 34 50 45 47C41 53 35 56 29 55C16 54 9 43 11 30C13 18 23 9 36 9Z',8],
 ['Sun','#f6ebd1','#d7ad59','#624c28','M32 6L37 14L45 10L47 19L56 21L52 29L58 36L50 41L51 50L42 50L37 58L30 53L22 58L18 50L9 49L12 40L6 34L13 28L10 20L19 19L22 11L28 15Z',9],
 ['Pebble','#e4e4df','#8d9a89','#3d5148','M12 26C14 15 25 9 37 12C48 14 54 24 52 36C51 48 43 54 30 53C16 52 9 43 12 26Z',0],
 ['Petal','#f0e2ec','#ae80a1','#583e55','M32 7C35 16 42 18 51 16C48 25 50 31 57 36C48 38 44 44 44 53C36 49 28 50 21 55C20 46 15 41 7 38C14 32 16 25 13 17C22 19 29 15 32 7Z',1],
 ['Sprout','#dce9df','#74a783','#355d47','M18 20Q18 11 27 11H37Q46 11 46 20V45Q46 53 38 53H26Q18 53 18 45Z',2],
 ['Wave','#e0ebf2','#6395ad','#305062','M10 22C14 12 22 10 29 17C35 8 45 9 49 18C56 20 57 29 52 34C56 45 49 53 40 50C35 56 25 54 22 49C12 50 8 43 12 35C7 30 7 25 10 22Z',3],
 ['Arc','#ede5d8','#bc9870','#5b4939','M10 50V31C10 17 19 9 32 9S54 17 54 31V50Q54 54 50 54H14Q10 54 10 50Z',4],
 ['Star','#e6e0ee','#9c85b0','#493c5e','M32 7L38 21L53 15L47 29L58 38L43 39L40 55L30 45L18 55L19 40L6 34L19 27L16 13L27 20Z',5],
 ['Drop','#d8e9ed','#6fa8b2','#31535d','M32 7C27 16 13 26 13 38C13 49 21 56 32 56S51 49 51 38C51 26 37 16 32 7Z',6],
 ['Tile','#f0e4dc','#c18f7e','#67463f','M12 17Q12 10 19 10H45Q52 10 52 17V47Q52 54 45 54H19Q12 54 12 47Z',7],
 ['Kite','#e3ebd9','#9aad77','#48583a','M32 7L53 31L32 57L11 31Z',8],
 ['Spiral','#eae0db','#b58178','#59423e','M12 35C12 20 23 10 35 11C47 12 54 21 53 33C52 47 42 54 30 53C19 52 12 45 12 35Z',9]
];




document.querySelector('#closeChatDetails').onclick=()=>document.querySelector('#chatDetailsDialog').close();document.querySelector('#cancelChatDetails').onclick=()=>document.querySelector('#chatDetailsDialog').close();


taskOverviewToggle.onclick=()=>setInspector('overview');changesToggle.onclick=()=>setInspector('changes');document.querySelector('#closeInspector').onclick=()=>setInspector(inspectorMode);refreshChanges.onclick=()=>loadChanges();















// The client only identifies candidate blocks; the server validates every one.
// Ambiguous markers fail closed and leave the original response in the editor.




document.querySelector('#proposeEditButton').onclick=openEditProposal;

// Only references and paths are stored in browser storage; file contents stay on the local server.




const recentWebUndos=loadUndoReferences('isa.webUndoRefs.v1');
// Apply one exact proposal through the authenticated supervisor; no model or
// browser-provided risk label grants write authority.


document.querySelector('#undoEditButton').onclick=openWebUndo;
const recentTransactionUndos=loadUndoReferences('isa.transactionUndoRefs.v1');

document.querySelector('#transactionButton').onclick=()=>openTransaction(false);
document.querySelector('#undoTransactionButton').onclick=()=>openTransaction(true);


document.querySelector('#newTask').onclick=newChat;


const approvalModes={always:'Ask every time',risk:'Ask occasionally',free:'Never ask'};
const approvalButton=document.querySelector('#approvalModeButton'),approvalMenu=document.querySelector('#approvalModeMenu');



approvalButton.addEventListener('click',()=>{renderApprovalMode();approvalMenu.hidden=!approvalMenu.hidden;approvalButton.setAttribute('aria-expanded',String(!approvalMenu.hidden));if(!approvalMenu.hidden)approvalMenu.querySelector('[aria-checked="true"]').focus()});
const unrestrictedDialog=document.querySelector('#unrestrictedDialog');let pendingUnrestricted=null;

approvalMenu.addEventListener('click',event=>{const button=event.target.closest('[data-approval-mode]');if(!button)return;const next=button.dataset.approvalMode;if(next==='free'&&next!==currentApprovalMode()){pendingUnrestricted={chatId:activeChatId,draft:draftChatSettings,mode:currentApprovalMode()};closeApprovalMenu();if(!unrestrictedDialog.open)unrestrictedDialog.showModal();return}if(saveApprovalMode(next))closeApprovalMenu(true)});
document.querySelector('#cancelUnrestricted').addEventListener('click',()=>unrestrictedDialog.close());
document.querySelector('#confirmUnrestricted').addEventListener('click',()=>{const target=pendingUnrestricted;if(!target||target.chatId!==activeChatId||(!activeChatId&&target.draft!==draftChatSettings)||target.mode!==currentApprovalMode()){isaNotifications.show('Chat or supervision setting changed. Review it again.');unrestrictedDialog.close();return}if(saveApprovalMode('free'))unrestrictedDialog.close()});
unrestrictedDialog.addEventListener('close',()=>{pendingUnrestricted=null;approvalButton.focus()});
approvalMenu.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();closeApprovalMenu(true);return}if(event.key==='ArrowDown'||event.key==='ArrowUp'){event.preventDefault();const items=[...approvalMenu.querySelectorAll('[data-approval-mode]')];const index=items.indexOf(document.activeElement);items[(index+(event.key==='ArrowDown'?1:items.length-1))%items.length].focus()}});
document.addEventListener('click',event=>{if(!approvalMenu.hidden&&!event.target.closest('.approval-control'))closeApprovalMenu()});renderApprovalMode();

// Manual execution is opt-in on a loopback-only server. The browser never accepts
// model text as authority to run an action. Secrets remain in tab session memory.
let localExecKey='';
// Never transmit this capability to an Arena preview or any non-loopback origin.

// Best-effort encrypted audit contains numeric usage only, never provider credentials.




document.querySelector('#runCommandButton').onclick=()=>openLocalCommand();
// A built-in read-only action uses the same server-side action supervisor as Bash.
// Models and Agent Pools cannot invoke this action.
document.querySelector('#workspaceInfoButton').onclick=()=>{
  if(location.protocol!=='http:'||location.hostname!=='127.0.0.1'){isaNotifications.show('Local workspace information requires opening ISA directly at http://127.0.0.1. Do not enter the token in a remote preview.');return}
  const chatId=activeChatId;if(!chatId){isaNotifications.show('Start a chat before inspecting the workspace.');return}
  const dialog=node('dialog','routing-consent'),form=node('form'),label=node('label','','Local execution token'),key=node('input'),status=node('p'),details=node('p'),actions=node('div','dialog-actions'),close=node('button','secondary-action','Close'),submit=node('button','primary','Inspect workspace');
  key.type='password';key.autocomplete='off';key.maxLength=256;key.required=true;key.value=localExecKey;label.append(key);status.setAttribute('role','status');close.type='button';submit.type='submit';actions.append(close,submit);form.append(label,status,details,actions);dialog.append(node('h2','','Inspect local workspace'),node('p','','Read the path and directory status of the server-configured workspace. The result is saved in this chat and may be sent to a model in later messages. This does not inspect files or run commands.'),form);document.body.append(dialog);
  let busy=false,closed=false;function finish(){closed=true;dialog.close();dialog.remove()}close.onclick=finish;dialog.oncancel=e=>{e.preventDefault();finish()};
  function same(mode){return !closed&&activeChatId===chatId&&currentApprovalMode()===mode&&!!chats.find(c=>c.id===chatId)}
  async function post(path,data){const response=await fetch(path,{method:'POST',headers:{'Content-Type':'application/json','X-ISA-Exec-Token':key.value},body:JSON.stringify(data)});const result=await response.json();isaNotifications.reportAuditWarning(result);if(!response.ok)throw Error(result.error||'Local action failed.');return result}
  form.onsubmit=async e=>{e.preventDefault();if(busy||!key.value)return;const mode=currentApprovalMode(),token=key.value;if(!same(mode))return;busy=true;key.disabled=submit.disabled=true;status.textContent='Registering chat policy…';
    try{
      const policy=await post('/api/exec/policy',{chat:chatId,mode});if(!same(mode))throw Error('Chat or supervision mode changed. No action was started.');
      const p=await post('/api/actions/prepare',{chat:chatId,action:{type:'workspace_info'}});if(!same(mode)){try{await post('/api/actions/cancel',{chat:chatId,id:p.id})}catch{}throw Error('Chat or supervision mode changed. No action was started.')}
      if(policy.mode!==mode||policy.executionProfile!==p.executionProfile||!['host','limited','sandboxed'].includes(p.executionProfile)||p.mode!==mode||typeof p.id!=='string'||typeof p.workspace!=='string'||p.action?.type!=='workspace_info'||Object.keys(p.action).length!==1||p.requiresApproval!==(mode==='always'))throw Error('Invalid supervisor response. No action was started.');
      localExecKey=token;details.textContent='Action: workspace info · Workspace: '+p.workspace;
      if(p.requiresApproval){status.textContent='Approval required for this read-only action.';const deny=node('button','secondary-action','Deny'),approve=node('button','primary','Approve and inspect');deny.type=approve.type='button';actions.replaceChildren(deny,approve);const approved=await new Promise(resolve=>{deny.onclick=()=>resolve(false);approve.onclick=()=>resolve(true);close.onclick=()=>{finish();resolve(false)};dialog.oncancel=e=>{e.preventDefault();finish();resolve(false)}});if(!approved||!same(mode)){try{await post('/api/actions/cancel',{chat:chatId,id:p.id,denied:!approved&&!closed&&same(mode)})}catch{}if(!closed)status.textContent='Denied or changed. No action was started.';return}}
      if(!same(mode))throw Error('Chat or supervision mode changed. No action was started.');actions.replaceChildren(close);close.onclick=finish;status.textContent='Inspecting workspace…';const response=await post('/api/actions/run',{chat:chatId,id:p.id,approved:p.requiresApproval}),result=response.result;if(!result||result.workspace!==p.workspace||result.isDirectory!==true)throw Error('Invalid workspace information result.');
      const current=chats.find(c=>c.id===chatId);let saved=false;if(current){const message={role:'assistant',provider:'local-tool',text:'Local workspace info (untrusted data):\n'+JSON.stringify(result,null,2),error:false,at:new Date().toISOString()};saved=saveChats(chats.map(c=>c.id===chatId?{...c,messages:[...c.messages,message]}:c));if(activeChatId===chatId&&!thread.hidden)openChat(chatId)}if(!closed)status.textContent=saved?'Finished. The result was saved in this chat and may be sent to a model later.':'Finished, but the result could not be saved.';
    }catch(error){if(!closed)status.textContent=error.message||'Local action failed.'}finally{busy=false;if(!closed){key.disabled=submit.disabled=false;actions.replaceChildren(close,submit);close.onclick=finish}}
  };dialog.showModal();key.focus();
};
// Manual MCP session; model and pool responses never authorize or invoke tools.
document.querySelector('#mcpToolsButton').onclick=()=>{
  if(location.protocol!=='http:'||location.hostname!=='127.0.0.1'){isaNotifications.show('MCP requires opening ISA directly at http://127.0.0.1. Do not enter the token in a remote preview.');return}
  const chatId=activeChatId;if(!chatId){isaNotifications.show('Start a chat before using MCP.');return}
  const dialog=node('dialog','routing-consent'),form=node('form'),label=node('label','','Local execution token'),key=node('input'),status=node('p'),review=node('pre'),toolLabel=node('label','','Discovered tool'),select=node('select'),argLabel=node('label','','Arguments (JSON object)'),args=node('textarea'),metadata=node('pre'),actions=node('div','dialog-actions'),close=node('button','secondary-action','Close'),submit=node('button','primary','Prepare server');
  key.type='password';key.autocomplete='off';key.maxLength=256;key.required=true;key.value=localExecKey;label.append(key);args.value='{}';args.maxLength=4096;toolLabel.append(select);argLabel.append(args);toolLabel.hidden=argLabel.hidden=true;status.setAttribute('role','status');for(const el of [review,metadata]){el.style.whiteSpace='pre-wrap';el.style.overflowWrap='anywhere';el.style.maxHeight='180px';el.style.overflow='auto'}close.type='button';submit.type='submit';actions.append(close,submit);form.append(label,status,review,toolLabel,argLabel,metadata,actions);dialog.append(node('h2','','Local MCP tools'),node('p','','Starting or listing the configured server can modify files and transmit data. Tools can do the same. The server has network access. Host and limited profiles are not sandboxed; the optional bubblewrap profile has important isolation limitations. Tool metadata and results are untrusted. This manual session does not share them with models or pools.'),form);document.body.append(dialog);
  let busy=false,closed=false,session=null,tools=[],fixedToken='';
  async function post(path,data,token=key.value){const response=await fetch(path,{method:'POST',headers:{'Content-Type':'application/json','X-ISA-Exec-Token':token},body:JSON.stringify(data)});let result;try{result=await response.json()}catch{throw Error('Invalid server response. The action may have run.')};isaNotifications.reportAuditWarning(result);if(!response.ok)throw Error(result.error||'MCP request failed.');return result}
  function closeSession(id){if(id&&fixedToken)post('/api/actions/mcp/close',{chat:chatId,session:id},fixedToken).catch(()=>{})}
  function finish(){if(closed)return;closed=true;closeSession(session);dialog.close();dialog.remove()}close.onclick=finish;dialog.oncancel=e=>{e.preventDefault();finish()};
  const same=mode=>!closed&&activeChatId===chatId&&currentApprovalMode()===mode&&!!chats.find(c=>c.id===chatId);
  const safeDisplay=value=>JSON.stringify(value,null,2).replace(/[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/g,'�');
  const canonical=value=>JSON.stringify(value,(k,v)=>v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.entries(v).sort(([a],[b])=>a<b?-1:a>b?1:0)):v);
  select.onchange=()=>{const chosen=tools.find(t=>t.name===select.value);metadata.textContent=chosen?'Untrusted tool metadata (experimental server schema validation; unsupported schemas are rejected):\n'+safeDisplay(chosen):''};
  form.onsubmit=async e=>{e.preventDefault();if(busy||!key.value)return;const mode=currentApprovalMode(),token=key.value;if(!same(mode))return;let action;
    if(!session)action={type:'mcp_start'};else{let value;try{value=JSON.parse(args.value)}catch{status.textContent='Arguments must be a JSON object.';return}if(!value||Array.isArray(value)||typeof value!=='object'){status.textContent='Arguments must be a JSON object.';return}action={type:'mcp_call',session,name:select.value,arguments:value}}
    busy=true;key.disabled=submit.disabled=select.disabled=args.disabled=true;status.textContent='Preparing supervised action…';let pending=null;
    try{
      const policy=await post('/api/exec/policy',{chat:chatId,mode},token);if(!same(mode))throw Error('Chat or supervision mode changed. Nothing started.');
      pending=await post('/api/actions/prepare',{chat:chatId,action},token);if(!same(mode)){await post('/api/actions/cancel',{chat:chatId,id:pending.id},token).catch(()=>{});throw Error('Chat or supervision mode changed. Nothing started.')}
      if(policy.mode!==mode||policy.executionProfile!==pending.executionProfile||!['host','limited','sandboxed'].includes(pending.executionProfile)||pending.mode!==mode||typeof pending.id!=='string'||typeof pending.workspace!=='string'||!Array.isArray(pending.serverCommand)||!pending.serverCommand.every(x=>typeof x==='string')||canonical(pending.action)!==canonical(action)||pending.requiresApproval!==(mode!=='free')||(action.type==='mcp_call'&&(![1,2].includes(pending.maxAttempts)||pending.maxAttempts!==(tools.find(t=>t.name===action.name)?.annotations?.idempotentHint===true?2:1))))throw Error('Invalid supervisor response. Nothing started.');
      fixedToken=token;localExecKey=token;review.textContent='Workspace: '+pending.workspace+'\nExecution profile: '+pending.executionProfile+(pending.executionProfile==='sandboxed'?' (bubblewrap; host network enabled)':' (not a sandbox)')+'\nServer command: '+safeDisplay(pending.serverCommand)+'\nAction: '+safeDisplay(action)+(action.type==='mcp_call'?'\nMaximum attempts: '+pending.maxAttempts+'. A declared-idempotent tool may be repeated after a tool error; its server can be wrong and effects may duplicate.':'');
      status.textContent=pending.requiresApproval?'Approval required: this action is high-risk.':'Review the exact action before explicitly starting it.';const deny=node('button','secondary-action','Deny'),approve=node('button','primary',pending.requiresApproval?'Approve action':'Start action');deny.type=approve.type='button';actions.replaceChildren(deny,approve);
      const approved=await new Promise(resolve=>{deny.onclick=()=>resolve(false);approve.onclick=()=>resolve(true);close.onclick=()=>{finish();resolve(false)};dialog.oncancel=e=>{e.preventDefault();finish();resolve(false)}});
      if(!approved||!same(mode)){await post('/api/actions/cancel',{chat:chatId,id:pending.id,denied:!approved&&!closed&&same(mode)},token).catch(()=>{});if(!closed)status.textContent='Denied or changed. Nothing started.';return}
      actions.replaceChildren(close);close.onclick=finish;status.textContent='Action running; closing cannot undo its effects…';const response=await post('/api/actions/run',{chat:chatId,id:pending.id,approved:pending.requiresApproval},token);
      if(action.type==='mcp_start'){
        const result=response.result;if(!result||typeof result.session!=='string'||!Array.isArray(result.tools)||!result.tools.every(t=>t&&typeof t.name==='string'))throw Error('Invalid discovery response; server actions may have occurred.');session=result.session;if(!same(mode)){closeSession(session);session=null;throw Error('Chat or supervision changed; session closed. Server actions may have occurred.')}
        tools=result.tools;select.replaceChildren();for(const tool of tools){const option=node('option','',tool.name);option.value=tool.name;select.append(option)}toolLabel.hidden=argLabel.hidden=false;submit.textContent='Prepare tool call';select.onchange();status.textContent=tools.length?'Discovery complete. Choose a tool and review its arguments. One call per session.':'No tools discovered. Close this session.';if(!tools.length)submit.disabled=true;
      }else{session=null;if(!closed){metadata.textContent='Untrusted tool result (not sent to a model):\n'+safeDisplay(response.result);status.textContent='Tool call completed. The session is closed. Start a new session for another call.';toolLabel.hidden=argLabel.hidden=true;submit.textContent='Prepare server'}}
    }catch(error){if(action.type==='mcp_call'){closeSession(session);session=null;if(!closed){toolLabel.hidden=argLabel.hidden=true;submit.textContent='Prepare server'}}if(!closed)status.textContent=(error.message||'MCP request failed.')+' If started, the action may have occurred; do not blindly retry.'}
    finally{busy=false;if(!closed){key.disabled=false;select.disabled=args.disabled=false;submit.disabled=!!session&&!tools.length;actions.replaceChildren(close,submit);close.onclick=finish}}
  };dialog.showModal();key.focus();
};
// Bounded model-driven loop. Approval of local actions is independent of
// consent for each outbound provider request. No model output is authority.

document.querySelector('#toolLoopButton').onclick=()=>openToolLoop();
// Explicit, single-agent pool MCP interaction. Nothing is shared without a
// separate consent, and the ordinary multi-agent workflow remains unchanged.

document.querySelector('#poolMcpButton').onclick=()=>openPoolMcp();
document.querySelector('#chatMcpButton').onclick=()=>openPoolMcp(true);
// Read-only view of the supervisor's volatile, redacted event history.
document.querySelector('#actionHistoryButton').onclick=()=>{
  if(location.protocol!=='http:'||location.hostname!=='127.0.0.1'){isaNotifications.show('Action history requires opening ISA directly at http://127.0.0.1. Do not enter the token in a remote preview.');return}
  const chatId=activeChatId;if(!chatId){isaNotifications.show('Start a chat before viewing action history.');return}
  const dialog=node('dialog','routing-consent'),form=node('form'),label=node('label','','Local execution token'),key=node('input'),status=node('p'),list=node('ol'),detailList=node('div'),actions=node('div','dialog-actions'),close=node('button','secondary-action','Close'),refresh=node('button','primary','Load history');key.type='password';key.autocomplete='off';key.maxLength=256;key.required=true;key.value=localExecKey;label.append(key);status.setAttribute('role','status');list.style.maxHeight='270px';list.style.overflow='auto';close.type='button';refresh.type='submit';actions.append(close,refresh);form.append(label,status,list,detailList,actions);dialog.append(node('h2','','Local action history'),node('p','','Recent redacted events are stored in .isa/web-audit in the execution workspace when available. This is best effort, not tamper-proof and not proof of human approval. It can be incomplete after failures or crashes; the display shows at most 100 events. Encrypted details, when explicitly enabled, can contain sensitive data even after heuristic redaction; reported token usage is recorded only when encrypted auditing and a local token are available. Credentials must not be deliberately supplied. An unknown outcome means the action may have had effects.'),form);document.body.append(dialog);let closed=false,busy=false;function finish(){closed=true;dialog.close();dialog.remove()}close.onclick=finish;dialog.oncancel=e=>{e.preventDefault();finish()};
  const kinds={shell:'Shell command',edit:'File edit',edit_undo:'Undo file edit',edit_transaction:'Multi-file edit',edit_transaction_undo:'Undo multi-file edit',semantic_search:'Semantic index search',work_start:'Start work baseline',work_status:'Work tracker status',work_end:'Delete work baseline',running_actions:'Shell action status',stop_shell:'Stop shell action',workspace_info:'Workspace info',mcp_start:'MCP startup and discovery',mcp_call:'MCP tool call'},statuses={proposed:'Proposed',approved:'Approved',authorized_without_prompt:'Authorized without prompt',denied:'Denied',cancelled:'Cancelled before dispatch',policy_changed:'Invalidated by policy change',expired:'Expired',rejected:'Rejected',retrying:'Retrying declared-idempotent tool',not_started:'Not started',dispatched:'Dispatched',completed:'Completed',failed:'Failed',unknown:'Outcome unknown'};
  form.onsubmit=async e=>{e.preventDefault();if(busy||!key.value)return;busy=true;refresh.disabled=key.disabled=true;status.textContent='Loading local history…';list.replaceChildren();detailList.replaceChildren();try{const response=await fetch('/api/actions/events',{method:'POST',headers:{'Content-Type':'application/json','X-ISA-Exec-Token':key.value},body:JSON.stringify({chat:chatId})}),data=await response.json();if(!response.ok)throw Error(data.error||'Could not load history.');if(!Array.isArray(data.events)||data.events.length>100)throw Error('Invalid history response.');if(closed||activeChatId!==chatId){if(!closed)status.textContent='The active chat changed. Close and reopen history.';return}for(const event of [...data.events].reverse()){if(!event||!Object.hasOwn(kinds,event.kind)||!Object.hasOwn(statuses,event.status)||!['manual','pool'].includes(event.source)||typeof event.actionRef!=='string'||!/^[a-f0-9]{48}$/.test(event.actionRef)||!Number.isSafeInteger(event.unixSeconds)||event.unixSeconds<0||event.unixSeconds>8640000000000)throw Error('Invalid history response.');const date=new Date(event.unixSeconds*1000);if(Number.isNaN(date.getTime()))throw Error('Invalid history response.');list.append(node('li','',date.toLocaleString()+' · '+kinds[event.kind]+' · '+statuses[event.status]+' · '+event.source+' · '+event.actionRef.slice(0,8)))}if(data.encryptedDetails!==undefined){if(!['disabled','best_effort','degraded'].includes(data.encryptedDetails)||!Array.isArray(data.details)||data.details.length>20)throw Error('Invalid encrypted audit response.');detailList.append(node('h3','','Encrypted action details'));for(const entry of data.details){if(!entry||typeof entry.actionRef!=='string'||!/^[a-f0-9]{48}$/.test(entry.actionRef)||!['arguments','result','usage'].includes(entry.kind)||typeof entry.payload!=='string'||new TextEncoder().encode(entry.payload).length>16384||typeof entry.truncated!=='boolean')throw Error('Invalid encrypted audit response.');const item=node('details'),summary=node('summary','',(entry.kind==='usage'?'Token usage ':'Action ')+entry.actionRef.slice(0,8)+' · '+entry.kind+(entry.truncated?' · truncated':'')),pre=node('pre','',entry.payload);pre.style.whiteSpace='pre-wrap';pre.style.overflowWrap='anywhere';item.append(summary,pre);detailList.append(item)}detailList.append(node('p','inspector-note',data.encryptedDetails==='best_effort'?(data.encryptedDetailsExperimental===true?'Experimental Windows encrypted details are unvalidated and may be incomplete.':'Encrypted details are best effort and may be incomplete.'):data.encryptedDetails==='degraded'?'Encrypted detail storage or key is unavailable; no plaintext fallback.':'Encrypted detail storage is disabled.'))}localExecKey=key.value;const persistence=data.persistence==='best_effort'&&data.volatile===false?'Persistent best-effort history; gaps and tampering remain possible.':data.persistence==='degraded'?'Audit persistence failed or is unsafe; showing only in-memory events. History may be missing.':'Server returned legacy or unknown persistence status; do not assume history is durable.';status.textContent=persistence+' '+(data.events.length?'Showing up to 100 recent events. A reference groups events for the same action.':'No recent events for this chat.');refresh.textContent='Refresh history'}catch(error){if(!closed){list.replaceChildren();detailList.replaceChildren();status.textContent=error.message||'Could not load history.'}}finally{busy=false;if(!closed){refresh.disabled=key.disabled=false}}};dialog.showModal();key.focus();
};
// UI for a future authenticated command-approval protocol. It never executes commands.

const archivedAttachments=new Map();let archivedBytes=0;

let attachments=[];let readingFiles=false;let attachmentGeneration=0;let selectedFolder=null;
const fileInput=document.querySelector('#fileInput');
const attachmentDialog=document.querySelector('#attachmentDialog'),attachmentPreview=document.querySelector('#attachmentPreview'),attachmentEditor=document.querySelector('#attachmentEditor');
const attachmentTitle=document.querySelector('#attachmentTitle'),attachmentDetail=document.querySelector('#attachmentDetail'),attachmentHint=document.querySelector('#attachmentHint'),saveAttachment=document.querySelector('#saveAttachment');
let currentAttachment=null,previewUrl=null,editorState=null,previewGeneration=0,activePdf=null;
const MAX_ATTACHMENT_BYTES=10*1024*1024,MAX_ATTACHMENT_TOTAL=25*1024*1024,MAX_ATTACHMENTS=8;



document.querySelector('#addContext').onclick=()=>fileInput.click();

let pdfLibraryPromise=null,mammothPromise=null;









fileInput.addEventListener('change',()=>{const files=Array.from(fileInput.files).map(file=>({file,name:file.name}));fileInput.value='';addContextFiles(files)});
promptBox.addEventListener('paste',event=>{const items=Array.from(event.clipboardData?.items||[]);const images=items.filter(item=>item.kind==='file'&&item.type.startsWith('image/')).map(item=>item.getAsFile()).filter(Boolean);if(!images.length)return;if(!items.some(item=>item.kind==='string'&&item.type==='text/plain'))event.preventDefault();addContextFiles(images.map((file,index)=>({file,name:file.name||('pasted-image-'+(index+1)+'.png')})))});

attachmentDialog.addEventListener('close',resetPreview);
document.querySelector('#closeAttachment').onclick=()=>attachmentDialog.close();document.querySelector('#cancelAttachment').onclick=()=>attachmentDialog.close();



document.querySelector('#undoStroke').onclick=()=>{if(editorState){editorState.strokes.pop();renderStrokes()}};document.querySelector('#clearStrokes').onclick=()=>{if(editorState){editorState.strokes=[];renderStrokes()}};
saveAttachment.onclick=()=>{const state=editorState,file=currentAttachment,generation=previewGeneration;if(!state||!file)return;saveAttachment.disabled=true;state.canvas.toBlob(blob=>{saveAttachment.disabled=false;if(generation!==previewGeneration||currentAttachment!==file)return;if(!blob||blob.size>MAX_ATTACHMENT_BYTES){isaNotifications.show('Marked image could not be saved or exceeds 10 MB.');return}file.strokes=structuredClone(state.strokes);file.editedBlob=blob;revokeAttachment(file);file.thumbUrl=URL.createObjectURL(blob);renderAttachments();attachmentDialog.close();isaNotifications.show('Image marks saved locally.')},'image/png')};
document.querySelector('#downloadAttachment').onclick=()=>{const file=currentAttachment;if(!file)return;const blob=file.editedBlob||file.file;const url=URL.createObjectURL(blob);const link=document.createElement('a');link.href=url;link.download=file.editedBlob?file.name.replace(/\.[^.]+$/,'')+'-marked.png':file.name;document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),1000)};
const folderDialog=document.querySelector('#folderDialog'),folderList=document.querySelector('#folderList'),folderPath=document.querySelector('#folderPath'),folderInput=document.querySelector('#folderInput');
let folderStack=[];let folderRenderToken=0;




document.querySelector('#folderPicker').onclick=()=>{folderDialog.showModal();renderFolder()};
document.querySelector('#closeFolder').onclick=()=>folderDialog.close();
document.querySelector('#removeFolder').onclick=()=>{selectedFolder=null;folderStack=[];renderFolder();isaNotifications.show('Folder removed from this session.')};
document.querySelector('#chooseFolder').onclick=async()=>{if(!window.showDirectoryPicker){isaNotifications.show('Use the file picker to select a folder in this browser.');return}try{const handle=await window.showDirectoryPicker({mode:'read'});selectedFolder={kind:'directory',name:handle.name,handle};folderStack=[{name:handle.name,handle}];renderFolder()}catch(error){if(error.name!=='AbortError')isaNotifications.show('Folder access unavailable. Try the file picker.')}};
document.querySelector('#fallbackFolder').onclick=()=>folderInput.click();
folderInput.addEventListener('change',()=>{const files=Array.from(folderInput.files);folderInput.value='';if(!files.length)return;const root=files[0].webkitRelativePath.split('/')[0]||'Selected folder';selectedFolder={kind:'files',name:root,files:files.slice(0,2000)};folderStack=[];renderFolder();if(files.length>2000)isaNotifications.show('Showing the first 2,000 files.')});
const providers=[
  {name:'GitHub',asset:'github',description:'Work with repositories, issues, and pull requests.'},
  {name:'Google Drive',asset:'google-drive',description:'Find and manage files in Google Drive.'},
  {name:'Google Docs',asset:'google-docs',description:'Create, read, and edit documents.'},
  {name:'Google Sheets',asset:'google-sheets',description:'Work with spreadsheets.'},
  {name:'Google Slides',asset:'google-slides',description:'Read and edit presentations.'},
  {name:'Notion',asset:'notion',description:'Look up your documents and notes.'},
  {name:'Linear',asset:'linear',description:'Track issues and projects.'},
  {name:'Gmail',asset:'gmail',description:'Connect your email account.'},
  {name:'Supabase',asset:'supabase',description:'Work with your Supabase projects.'},
  {name:'Vercel',asset:'vercel',description:'Access your Vercel projects.'},
  {name:'Dropbox',asset:'dropbox',description:'Find and manage Dropbox files.'},
  {name:'Google Calendar',asset:'google-calendar',description:'Connect your calendar.'}
];
const connectionsPopover=document.querySelector('#connectionsPopover'),connectionsButton=document.querySelector('#attachFile'),providersDialog=document.querySelector('#providersDialog');




connectionsButton.onclick=()=>{const open=connectionsPopover.hidden;connectionsPopover.hidden=!open;connectionsButton.setAttribute('aria-expanded',String(open));if(open)renderProviders()};
document.querySelector('#viewProviders').onclick=()=>{closeConnections();renderProviders();document.querySelector('#providerStatus').textContent='Connections are not available yet.';providersDialog.showModal()};
document.querySelector('#closeProviders').onclick=()=>providersDialog.close();
document.addEventListener('click',event=>{if(!connectionsPopover.hidden&&!connectionsPopover.contains(event.target)&&!connectionsButton.contains(event.target))closeConnections()});
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!connectionsPopover.hidden){closeConnections();connectionsButton.focus()}});
const SpeechRecognition=window.SpeechRecognition||window.webkitSpeechRecognition;const voiceButton=document.querySelector('#voiceInput');
if(!SpeechRecognition){voiceButton.disabled=true;voiceButton.title='Speech-to-text is not supported by this browser.'}else{let recognition=null,asking=false;stopVoiceOnNavigate=()=>{if(recognition)recognition.stop()};voiceButton.onclick=async()=>{if(recognition){recognition.stop();return}if(asking)return;asking=true;const voiceChat=activeChatId,voiceSession=voiceContext,accepted=await confirmVoice();asking=false;if(!accepted||activeChatId!==voiceChat||voiceContext!==voiceSession)return;recognition=new SpeechRecognition();const current=recognition;current.lang=navigator.language||'en-US';current.interimResults=false;current.onresult=event=>{if(activeChatId!==voiceChat||voiceContext!==voiceSession){isaNotifications.show('Voice input stopped because the conversation changed.');current.stop();return}let spoken='';for(let i=event.resultIndex;i<event.results.length;i++)if(event.results[i].isFinal)spoken+=(spoken?' ':'')+event.results[i][0].transcript;if(spoken){const start=promptBox.selectionStart,end=promptBox.selectionEnd,before=promptBox.value.slice(0,start),after=promptBox.value.slice(end),insert=(before&&!/\s$/.test(before)?' ':'')+spoken+(after&&!/^\s/.test(after)?' ':'');promptBox.setRangeText(insert,start,end,'end');promptBox.dispatchEvent(new Event('input',{bubbles:true}));promptBox.focus()}};current.onerror=event=>isaNotifications.show('Voice input: '+event.error);current.onend=()=>{if(recognition===current)recognition=null;voiceButton.classList.remove('recording');voiceButton.setAttribute('aria-label','Dictate message')};try{current.start();voiceButton.classList.add('recording');voiceButton.setAttribute('aria-label','Stop dictation')}catch(error){recognition=null;voiceButton.classList.remove('recording');isaNotifications.show('Could not start voice input.')}}}
function confirmVoice(){return new Promise(resolve=>{const dialog=node('dialog','routing-consent'),actions=node('div','dialog-actions'),cancel=node('button','secondary-action','Cancel'),start=node('button','primary','Start dictation');cancel.type=start.type='button';actions.append(cancel,start);dialog.append(node('h2','','Voice input'),node('p','','Your browser may send microphone audio to its speech recognition service. Browser and microphone permissions are required. Review the text before sending.'),actions);document.body.append(dialog);let done=false;function finish(value){if(done)return;done=true;dialog.close();dialog.remove();resolve(value)}cancel.onclick=()=>finish(false);start.onclick=()=>finish(true);dialog.oncancel=e=>{e.preventDefault();finish(false)};dialog.showModal();cancel.focus()})}


document.querySelector('#closeHelp').onclick=()=>document.querySelector('#helpDialog').close();
const helpDescriptions={
  'isabel te amo':'Shows a local, predefined response from Isabel.',
  'help':'Opens this guide without adding a message to chat.',
  'resume-ac':'Shows the 200–300 word all-chat summary when the CLI session is connected. The web chat cannot access it yet.',
  'resume-lp':'Shows the 100–200 word latest-prompt summary when the CLI session is connected. The web chat cannot access it yet.',
  'workspace-info':'Review information about the configured local workspace using chat supervision.',
  'mcp-tools':'Open manual local MCP startup and tool discovery.',
  'tool-loop':'Open the supervised model tool loop for a custom provider.',
  'chat-mcp':'Open the optional single-tool chat MCP workflow.',
  'pool-mcp':'Open the optional Agent Pool MCP workflow.',
  'action-history':'Show the local supervisor action history for this chat.',
  'running-actions':'Check registered supervised shell actions for this chat.',
  'process-snapshot':'Review a manually copied CLI process snapshot.',
  'capabilities':'Check the current server capability declaration.',
  'local-token':'Set or clear the local workspace token for this tab.',
  'edit-transaction':'Prepare and review a supervised multi-file edit.',
  'undo-transaction':'Prepare a supervised undo of a multi-file edit.',
  'undo-edit':'Prepare a supervised undo of a single-file edit.',
  'propose-edit':'Generate a read-only single-file edit proposal.',
  'projects':'Opens your projects saved in this browser.',
  'search':'Opens search for projects saved in this browser.',
  'agents':'Opens the Agents section to manage providers and pools.',
  'automations':'Opens the Automations section. Scheduling is not connected yet.',
  'extensions':'Opens the extensions catalog. Searching requires the Rust server and network access.',
  'history':'Opens conversations saved in this browser.',
  'knowledge':'Opens the Knowledge Center. Its backend is not connected yet.',
  'goal':'Manages goals in the Rust CLI. This command is not connected to the web chat yet.',
  'contract':'Manages subagent contracts in the Rust CLI. This command is not connected to the web chat yet.',
  'context':'Prepares project context in the Rust CLI. This command is not connected to the web chat yet.',
  'diff':'Shows tracked Git changes in the Rust CLI. This command is not connected to the web chat yet.',
  'work':'Start, inspect or end a supervised workspace change baseline.',
  'semantic-search':'Searches an existing CLI semantic index via local Ollama under chat supervision.',
  'sessions':'Lists and imports CLI sessions from the authenticated local execution workspace as new browser chats; it does not synchronize them.'
};
let helpPanelCounter=0;








const commands=[
  {name:'./isabel te amo/.',description:'Isabel’s response'},
  {name:'./workspace-info/.',description:'Local workspace information'},
  {name:'./mcp-tools/.',description:'Manual MCP tools'},
  {name:'./tool-loop/.',description:'Supervised model tool loop'},
  {name:'./chat-mcp/.',description:'Chat MCP workflow'},
  {name:'./pool-mcp/.',description:'Agent Pool MCP workflow'},
  {name:'./action-history/.',description:'Local action history'},
  {name:'./edit-transaction/.',description:'Review a multi-file edit'},
  {name:'./undo-transaction/.',description:'Undo a multi-file edit'},
  {name:'./undo-edit/.',description:'Undo a single-file edit'},
  {name:'./propose-edit/.',description:'Generate an edit proposal'},
  {name:'./agents/.',description:'Agents'},
  {name:'./automations/.',description:'Automations'},
  {name:'./context/.',description:'Project context'},
  {name:'./contract/.',description:'Subagent contracts'},
  {name:'./diff/.',description:'Git changes'},
  {name:'./extensions/.',description:'Extensions'},
  {name:'./goal/.',description:'Goals'},
  {name:'./help/.',description:'Help'},
  {name:'./history/.',description:'Chat history'},
  {name:'./knowledge/.',description:'Knowledge'},
  {name:'./projects/.',description:'Projects'},
  {name:'./resume/.',description:'Compress older context'},
  {name:'./resume-AC/.',description:'All-chat summary'},
  {name:'./resume-LP/.',description:'Last-prompt summary'},
  {name:'./search/.',description:'Search'},
  {name:'./sessions/.',description:'Import a local CLI session'},
  {name:'./semantic-search/.',description:'Search the CLI semantic index'},
  {name:'./work/.',description:'Workspace change baseline and counts'},
  {name:'./running-actions/.',description:'Supervised shell action status'},
  {name:'./process-snapshot/.',description:'Review a copied CLI process snapshot'},
  {name:'./capabilities/.',description:'Check the current server capabilities'},
  {name:'./local-token/.',description:'Set a local workspace token'}
].sort((a,b)=>a.name.localeCompare(b.name));
const completionGhost=document.querySelector('#completionGhost');
const completionHint=document.querySelector('#completionHint');
const commandList=document.querySelector('#commandList');
let completion='';let commandMatches=[];let selectedCommand=0;let commandWindowStart=0;


commandList.addEventListener('wheel',event=>{if(!commandMatches.length||event.deltaY===0)return;event.preventDefault();selectedCommand=Math.max(0,Math.min(commandMatches.length-1,selectedCommand+(event.deltaY>0?1:-1)));updateCompletion()},{passive:false});
promptBox.addEventListener('input',()=>updateCompletion(true));
promptBox.addEventListener('click',()=>updateCompletion());
promptBox.addEventListener('blur',()=>{completionGhost.hidden=true;completionHint.hidden=true;commandList.hidden=true;promptBox.setAttribute('aria-expanded','false');promptBox.removeAttribute('aria-activedescendant')});
promptBox.addEventListener('focus',()=>updateCompletion());
document.querySelector('#send').onclick=()=>{const stopLoop=activeChatId&&activeToolLoops.get(activeChatId);if(stopLoop){stopLoop();return}const running=activeChatId&&pendingChats.get(activeChatId);if(running){running.abort();return}send();updateCompletion()};
promptBox.addEventListener('keydown',event=>{
  if(event.key==='Tab'&&completion){event.preventDefault();acceptCompletion();return}
  if(completion&&(event.key==='ArrowDown'||event.key==='ArrowUp')){event.preventDefault();selectedCommand=Math.max(0,Math.min(commandMatches.length-1,selectedCommand+(event.key==='ArrowDown'?1:-1)));updateCompletion();return}
  if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();send();updateCompletion()}
});

const projectsSection=document.querySelector('#projects');
const projectGrid=document.querySelector('#projectGrid');
const projectDialog=document.querySelector('#projectDialog');
const projectSearch=document.querySelector('#projectSearch');
const projectStore=isaProjectStore.createProjectStore({getStorage:()=>localStorage});
if(!projectStore.load())isaNotifications.show('Saved projects could not be loaded.');






renderSidebarProjects();






document.querySelector('#projectsNav').onclick=showProjects;
document.querySelector('#createProject').onclick=openProjectDialog;
document.querySelector('#emptyCreate').onclick=openProjectDialog;
document.querySelector('#cancelProject').onclick=()=>projectDialog.close();
projectSearch.addEventListener('input',renderProjects);
document.querySelector('#projectForm').addEventListener('submit',event=>{event.preventDefault();const name=document.querySelector('#projectName').value.trim();if(!name){document.querySelector('#projectName').focus();return}const project={id:Date.now().toString(36)+'-'+Math.random().toString(36).slice(2),name,description:document.querySelector('#projectDescription').value.trim(),createdAt:new Date().toISOString(),chatIds:[]};if(!projectChange(projectStore.add(project)))return;projectDialog.close();projectSearch.value='';renderProjects();isaNotifications.show('Project created in this browser.')});

const hub=document.querySelector('#hub');
let extensionRequest=0;
const pages={
  'Agents':{symbol:'✧',title:'Agents',lead:'Manage providers and agent pools.'},
  'Folders':{symbol:'▱',title:'Folders',lead:'Access the folder selected for this browser session.',empty:'Workspace folder',detail:'Choose a folder to browse its files and add text to a chat. The agent is not connected yet.',action:'Choose a folder'},
  'Chats':{symbol:'☷',title:'Chats',lead:'Find and continue your conversations with ISA.',empty:'No saved conversations',detail:'Chat history is not connected in this preview. Messages in the demo are not saved.',action:'Start a new chat'},
  'Knowledge center':{symbol:'▤',title:'Knowledge center',lead:'Bring your project context and resources together.',empty:'Your knowledge center is empty',detail:'Documents and repository knowledge will appear here once a knowledge backend is connected.'},
  'Automations':{symbol:'◷',title:'Automations',lead:'Organize repeatable work and keep tasks moving.',empty:'No automations yet',detail:'Creating and scheduling automations requires the ISA runtime.'}
};
function node(tag,className,text){const element=document.createElement(tag);if(className)element.className=className;if(text!==undefined)element.textContent=text;return element}
function showHub(name,settingsCategory){
  activeChatId=null;sessionStorage.removeItem('isa.activeChat');setActiveSection(name);renderRecent();extensionRequest++;hub.replaceChildren();hub.classList.toggle('settings-view',name==='Settings');hub.classList.toggle('agents-view',name==='Agents');hub.hidden=false;projectsSection.hidden=true;welcome.hidden=true;thread.hidden=true;document.querySelector('.composer-wrap').hidden=true;document.querySelector('#title').textContent=name;isaSidebar.close();document.querySelector('#content').scrollTop=0;
  if(name==='Settings'){showSettings(settingsCategory);return}
  if(name==='Agents'){showAgents();return}
  if(name==='Chats'){showChats();return}
  if(name==='Extensions'){showExtensions();return}
  if(name==='Search'){showSearch();return}
  const page=pages[name];if(!page)return;
  hub.append(node('h1','',page.title),node('p','hub-lead',page.lead));
  const empty=node('div','hub-empty');empty.append(node('div','symbol',page.symbol),node('h2','',page.empty),node('p','',page.detail));
  if(page.action){const button=node('button','secondary-action',page.action);button.onclick=()=>name==='Folders'?document.querySelector('#folderPicker').click():document.querySelector('#newTask').click();empty.append(button)}hub.append(empty);
}
const agentPoolKey='isa.agentPools.v1';
let agentPools=[];
try{const saved=JSON.parse(localStorage.getItem(agentPoolKey)||'[]');if(Array.isArray(saved))agentPools=saved.filter(p=>p&&typeof p.id==='string'&&typeof p.name==='string'&&Array.isArray(p.members)&&Array.isArray(p.edges)).slice(0,100).map(p=>({id:p.id,name:p.name.slice(0,80),members:p.members.filter(id=>typeof id==='string').slice(0,50),edges:p.edges.filter(e=>e&&typeof e.from==='string'&&typeof e.to==='string').slice(0,200),graph:p.graph&&typeof p.graph==='object'?p.graph:null}))}catch(error){isaNotifications.show('Could not load agent pools.')}
function saveAgentPools(next){try{localStorage.setItem(agentPoolKey,JSON.stringify(next));agentPools=next;return true}catch(error){isaNotifications.show('Could not save agent pools in this browser.');return false}}
function parseProviderUsage(data){return isaProviderUtils.parseProviderUsage(data)}
function tokenStats(chat){return isaProviderUtils.tokenStats(chat)}
function renderTokens(){const menu=document.querySelector('#tokensMenu'),chat=chats.find(c=>c.id===activeChatId),stats=tokenStats(chat),messages=chat?.messages||[],checkpoint=Number.isInteger(chat?.contextSummary?.through)?chat.contextSummary.through:-1,contextParts=(checkpoint>=0?[chat.contextSummary.text||'']:[]).concat(messages.slice(checkpoint+1).filter(m=>!m.error).map(m=>m.text||'')),bytes=contextParts.reduce((n,text)=>n+new TextEncoder().encode(text).length,0),list=node('dl');menu.replaceChildren(node('h3','','Tokens · this conversation'));const display=value=>value.toLocaleString('en-US');for(const [label,value] of [['Context',messages.length?'≈ '+display(Math.ceil(bytes/4)):'—'],['Input',stats.known?display(stats.input):'—'],['Output',stats.known?display(stats.output):'—'],['Cached',stats.known>stats.cachedUnknown?display(stats.cached):'—'],['Total',stats.known?display(stats.total):'—']])list.append(node('dt','',label),node('dd','',value));menu.append(list,node('p','','Context is a rough text-only estimate, not a model tokenizer or context-window limit. Input and output are provider-reported totals across all calls, including pool calls; cached is included in input. Missing usage, failed or cancelled calls, and embedding calls may not be counted. '+(stats.unknown?stats.unknown+' request(s) have unknown usage.':'')+(stats.cachedUnknown?' Cached data is incomplete.':'')))}
const tokensButton=document.querySelector('#tokensButton'),tokensMenu=document.querySelector('#tokensMenu');tokensButton.onclick=()=>{const open=tokensMenu.hidden;tokensMenu.hidden=!open;tokensButton.setAttribute('aria-expanded',String(open));if(open)renderTokens()};document.addEventListener('click',event=>{if(!event.target.closest('.token-control')){tokensMenu.hidden=true;tokensButton.setAttribute('aria-expanded','false')}});document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!tokensMenu.hidden){tokensMenu.hidden=true;tokensButton.setAttribute('aria-expanded','false');tokensButton.focus()}});
// Provider routing and the bounded browser-side pool runner. No tools are invoked.
async function withRequestTimeout(parent,task,timeout=45000){
  if(parent.aborted)throw new DOMException('Stopped','AbortError');
  const child=new AbortController(),abort=()=>child.abort(),timer=setTimeout(()=>child.abort(),timeout);parent.addEventListener('abort',abort,{once:true});
  try{return await task(child.signal)}catch(error){if(parent.aborted)throw new DOMException('Stopped','AbortError');if(child.signal.aborted)return {text:'Provider request timed out.',error:true,retryable:true,transient:true};throw error}finally{clearTimeout(timer);parent.removeEventListener('abort',abort)}
}
async function timedCustomRequest(id,history,files,signal,options={},onUsage){let result;try{result=await withRequestTimeout(signal,child=>requestCustomProvider(id,history,files,child,options),options.poolCall?120000:45000);return result}finally{if(onUsage)onUsage(result?.usage||null)}}
async function requestServer(history,files,settings,signal,onUsage){let reported=false;try{return await withRequestTimeout(signal,async child=>{try{const response=await fetch('/api/chat',{method:'POST',headers:localWorkspaceHeaders(),body:JSON.stringify({message:history[history.length-1].text,messages:history,attachments:files,settings:{...settings,provider:'ollama'}}),signal:child});const data=await response.json();if(onUsage&&Array.isArray(data.usageCalls)&&data.usageCalls.length>0&&data.usageCalls.length<=32){for(const usage of data.usageCalls)onUsage(parseProviderUsage(usage));reported=true}if(['decoy','ollama'].includes(data.provider)&&data.role==='assistant'&&typeof data.error?.message==='string'&&data.error.message.length<=65536)return {text:data.error.message,error:true,provider:data.provider,retryable:true};if(response.ok&&data.provider==='ollama'&&data.role==='assistant'&&typeof data.content==='string'&&data.content.trim()&&data.content.length<=65536)return {text:data.content,error:false,provider:'ollama',activity:normalizeRunActivity(data.activity),agentSteps:normalizeAgentSteps(data.agentSteps)};return {text:'Server provider returned an invalid response.',error:true,retryable:response.status>=500}}catch(error){if(error.name==='AbortError')throw error;return {text:'Could not reach the server provider.',error:true,retryable:true}}},settings.agentLoop?480000:45000)}finally{if(onUsage&&!reported)onUsage(null)}}
async function requestAuto(history,files,settings,signal,onUsage){
  let result=await requestServer(history,files,settings,signal,onUsage);if(!result.error)return result;
  const failures=[];for(const provider of customProviders){if(signal.aborted)throw new DOMException('Stopped','AbortError');result=await timedCustomRequest(provider.id,history,files,signal,{},onUsage);if(!result.error)return {...result,text:'[Auto · '+provider.name+']\n\n'+result.text};failures.push(provider.name+': '+result.text);if(!result.retryable)return {...result,text:'Auto stopped at '+provider.name+': '+result.text}}
  return {text:failures.length?'No provider succeeded. '+failures.join(' '):result.text,error:true,provider:'auto'};
}



const routingButton=document.querySelector('#routingButton'),routingMenu=document.querySelector('#routingMenu');

routingButton.onclick=()=>{
  if(!routingMenu.hidden){closeRouting();return}
  const current=(chats.find(c=>c.id===activeChatId)?.settings||draftChatSettings).provider||'auto';routingMenu.replaceChildren();routingMenu.style.right='0px';
  function option(container,title,description,value){const button=node('button','');button.type='button';button.setAttribute('role','menuitemradio');button.setAttribute('aria-checked',String(current===value));button.append(node('strong','',title),node('small','',description));button.onclick=()=>{setChatRoute(value);closeRouting()};container.append(button)}
  option(routingMenu,'Auto','Tries the server and then saved providers on availability errors.','auto');
  const trigger=node('button','routing-sub-trigger','Agent pools');trigger.type='button';trigger.setAttribute('role','menuitem');trigger.setAttribute('aria-haspopup','menu');trigger.setAttribute('aria-expanded','false');trigger.setAttribute('aria-controls','routingPoolMenu');trigger.append(node('span','routing-chevron','›'));
  const sub=node('div','routing-submenu');sub.id='routingPoolMenu';sub.setAttribute('role','menu');sub.setAttribute('aria-label','Agent pools');sub.hidden=true;
  if(!agentPools.length)sub.append(node('p','','No agent pools yet. Create one in Agents.'));for(const pool of agentPools)option(sub,pool.name,'Run this workflow using its providers.','pool:'+pool.id);
  routingMenu.append(trigger,node('p','','Auto and agent pools may send your message and attachments to several providers and incur charges.'),sub);
  function toggleSub(open){sub.hidden=!open;trigger.setAttribute('aria-expanded',String(open));routingMenu.style.right='0px';sub.classList.remove('routing-sub-left','routing-sub-overlay');if(!open)return;const box=routingMenu.getBoundingClientRect(),subWidth=sub.getBoundingClientRect().width,availableRight=window.innerWidth-box.right-8,shift=Math.max(0,subWidth+6-availableRight);if(box.left-shift>=8)routingMenu.style.right=(shift/(box.width/routingMenu.offsetWidth||1))+'px';else if(box.left>=subWidth+14)sub.classList.add('routing-sub-left');else sub.classList.add('routing-sub-overlay')}
  trigger.onclick=()=>toggleSub(sub.hidden);trigger.onkeydown=event=>{if(event.key==='ArrowRight'){event.preventDefault();toggleSub(true);sub.querySelector('button')?.focus()}};sub.onkeydown=event=>{if(event.key==='ArrowLeft'||event.key==='Escape'){event.preventDefault();event.stopPropagation();toggleSub(false);trigger.focus()}};
  routingMenu.hidden=false;routingButton.setAttribute('aria-expanded','true');
};
document.addEventListener('click',event=>{if(!event.target.closest('.routing-control'))closeRouting()});document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!routingMenu.hidden){closeRouting();routingButton.focus()}});





// Compact only a bounded prefix. Original messages remain stored and visible.



// Goal text is untrusted reference, not executable authority. The operator must
// explicitly provide every command; the supervisor authorizes each separately.



// Normalize older team-only workflows while preserving their manager connections.


// Discovery only: inclusion does not imply a native integration or a verified connection.
const providerDirectory=['AI21 Labs','Alibaba Cloud Model Studio','Amazon Bedrock','Anthropic','Azure AI Foundry','Baseten','Cerebras','Claude (Anthropic)','Cloudflare Workers AI','Cohere','Databricks','Deepgram','DeepInfra','DeepSeek','Fireworks AI','FriendliAI','Google Gemini API','Google Vertex AI','Groq','Hugging Face','Hyperbolic','IBM watsonx.ai','Inception','Infermatic','Jina AI','Lepton AI','LM Studio','Mistral AI','Modal','Moonshot AI','Nebius AI Studio','Novita AI','Nscale','NVIDIA NIM','Ollama','OpenAI','OpenRouter','Oracle Cloud Infrastructure','Perplexity','Predibase','Replicate','RunPod','SambaNova','Scaleway','SiliconFlow','Snowflake Cortex','Together AI','Upstage','Vercel AI Gateway','xAI','xKiro','Z.ai'];
// Public chat-completions endpoints from provider documentation. These are editable
// suggestions, not proof of CORS support, credentials, or a successful connection.
const providerEndpoints={
  'AI21 Labs':'https://api.ai21.com/studio/v1/chat/completions',
  'Alibaba Cloud Model Studio':'https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions',
  'Anthropic':'https://api.anthropic.com/v1/chat/completions',
  'Baseten':'https://inference.baseten.co/v1/chat/completions',
  'Cerebras':'https://api.cerebras.ai/v1/chat/completions',
  'Claude (Anthropic)':'https://api.anthropic.com/v1/chat/completions',
  'Cohere':'https://api.cohere.ai/compatibility/v1/chat/completions',
  'DeepInfra':'https://api.deepinfra.com/v1/openai/chat/completions',
  'DeepSeek':'https://api.deepseek.com/chat/completions',
  'Fireworks AI':'https://api.fireworks.ai/inference/v1/chat/completions',
  'FriendliAI':'https://api.friendli.ai/serverless/v1/chat/completions',
  'Google Gemini API':'https://generativelanguage.googleapis.com/v1beta/openai/chat/completions',
  'Groq':'https://api.groq.com/openai/v1/chat/completions',
  'Hugging Face':'https://router.huggingface.co/v1/chat/completions',
  'Hyperbolic':'https://api.hyperbolic.ai/inference/v1/chat/completions',
  'Inception':'https://api.inceptionlabs.ai/v1/chat/completions',
  'Infermatic':'https://api.totalgpt.ai/v1/chat/completions',
  'Jina AI':'https://deepsearch.jina.ai/v1/chat/completions',
  'Mistral AI':'https://api.mistral.ai/v1/chat/completions',
  'Moonshot AI':'https://api.moonshot.ai/v1/chat/completions',
  'Nebius AI Studio':'https://api.studio.nebius.com/v1/chat/completions',
  'Novita AI':'https://api.novita.ai/openai/v1/chat/completions',
  'Nscale':'https://inference.api.nscale.com/v1/chat/completions',
  'NVIDIA NIM':'https://integrate.api.nvidia.com/v1/chat/completions',
  'Ollama':'https://ollama.com/v1/chat/completions',
  'OpenAI':'https://api.openai.com/v1/chat/completions',
  'OpenRouter':'https://openrouter.ai/api/v1/chat/completions',
  'Perplexity':'https://api.perplexity.ai/chat/completions',
  'SambaNova':'https://api.sambanova.ai/v1/chat/completions',
  'Scaleway':'https://api.scaleway.ai/v1/chat/completions',
  'SiliconFlow':'https://api.siliconflow.com/v1/chat/completions',
  'Together AI':'https://api.together.ai/v1/chat/completions',
  'Upstage':'https://api.upstage.ai/v1/chat/completions',
  'Vercel AI Gateway':'https://ai-gateway.vercel.sh/v1/chat/completions',
  'xAI':'https://api.x.ai/v1/chat/completions',
  'xKiro':'https://api.xkiro.com/v1/chat/completions',
  'Z.ai':'https://api.z.ai/api/paas/v4/chat/completions'
};




const chatSettingsDialog=document.querySelector('#chatSettingsDialog'),chatSettingsForm=document.querySelector('#chatSettingsForm'),chatProvider=document.querySelector('#chatProvider'),chatPluginsMode=document.querySelector('#chatPluginsMode'),chatAgentLoop=document.querySelector('#chatAgentLoop'),chatToolLoop=document.querySelector('#chatToolLoop'),poolSymbolQuery=document.querySelector('#poolSymbolQuery'),poolSymbolMode=document.querySelector('#poolSymbolMode'),chatPluginList=document.querySelector('#chatPluginList');
let settingsCatalog=[],settingsRequest=0;let ollamaAvailable=false,providerStatusKnown=false;

document.querySelector('#inspectWorkspaces').onclick=openWorkspaceBindings;

chatProvider.addEventListener('change',updateChatProviderStatus);

chatPluginsMode.addEventListener('change',updatePluginAvailability);

document.querySelector('#more').onclick=async()=>{const request=++settingsRequest;const current=chats.find(chat=>chat.id===activeChatId),settings=current?.settings||draftChatSettings;chatProvider.querySelectorAll('option[data-custom]').forEach(option=>option.remove());for(const provider of customProviders){const option=node('option','',provider.name+' · '+provider.model);option.value=provider.id;option.dataset.custom='true';chatProvider.append(option)}for(const pool of agentPools){const option=node('option','','Agent pool · '+pool.name);option.value='pool:'+pool.id;option.dataset.custom='true';chatProvider.append(option)}const chosen=typeof settings.provider==='string'?settings.provider:'auto';if((chosen.startsWith('custom-')&&!customProviders.some(provider=>provider.id===chosen))||(chosen.startsWith('pool:')&&!agentPools.some(pool=>pool.id===chosen.slice(5)))){const missing=node('option','',chosen.startsWith('pool:')?'Unavailable agent pool':'Unavailable custom provider');missing.value=chosen;missing.dataset.custom='true';chatProvider.append(missing)}chatProvider.value=Array.from(chatProvider.options).some(option=>option.value===chosen)?chosen:'auto';chatPluginsMode.checked=settings.pluginsMode!=='off';chatAgentLoop.checked=settings.agentLoop===true;chatToolLoop.checked=settings.toolLoopOnSend===true;poolSymbolQuery.value=typeof settings.poolSymbolQuery==='string'?settings.poolSymbolQuery:'';poolSymbolMode.value=settings.poolSymbolMode==='callers'?'callers':'declarations';document.querySelector('#agentLoopStatus').textContent='Read-only Ollama workspace search. Checking server configuration…';document.querySelector('#serverCapabilityStatus').textContent='Checking server capabilities…';updatePluginAvailability();settingsCatalog=[];chatPluginList.replaceChildren();document.querySelector('#chatPluginsStatus').textContent='Loading plugins…';document.querySelector('#chatProviderStatus').textContent='Checking available providers…';chatSettingsDialog.showModal();try{const response=await fetch('/api/chat-options',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'});if(!response.ok)throw Error('Unavailable');const data=await response.json();if(request!==settingsRequest||!chatSettingsDialog.open)return;document.querySelector('#serverCapabilityStatus').textContent=data.backendKind==='rust'?(data.localActionsPlatformSupported===false?(data.localActionsWindowsShellExperimental===true?'Rust server: untested experimental Windows PowerShell and workspace information only; event history volatile, encrypted details separately experimental; edits and undo unavailable.':data.localActionsWindowsReadOnly===true?'Rust server: experimental Windows read-only workspace information only; event history volatile, encrypted details separately experimental; commands and edits unavailable.':'Rust server: local supervised actions are unavailable on this server platform.'):data.localActionsPlatformSupported===true?'Rust server: platform supports local supervised actions; configuration and authentication are not verified.':'Rust server: local action platform support is unknown. Authentication is separate.'):data.backendKind==='node-preview'?'Node development preview: no local executor, Ollama, workspace inspection or MCP backend. Direct custom-provider requests may still work.':'Server capabilities cannot be identified. Do not assume local tools are available.';settingsCatalog=Array.isArray(data.catalog?.plugins)?data.catalog.plugins.filter(item=>typeof item.id==='string'&&Number.isFinite(item.count)):[];chatProvider.querySelector('[value="ollama"]').disabled=!data.ollamaConfigured;ollamaAvailable=!!data.ollamaConfigured;providerStatusKnown=true;document.querySelector('#poolSymbolStatus').textContent=data.symbolLookupAvailable?'Server workspace available. The selected lookup shares matching names, paths and line numbers with pool providers after confirmation; call sites are unresolved.':'Unavailable on this server: configure ISA_WEB_AGENT_WORKSPACE. The browser folder picker does not configure it.';document.querySelector('#agentLoopStatus').textContent=data.agentLoopAvailable?'Available for server Ollama. Searches send workspace excerpts to Ollama; no shell, edits, or sandbox.':'Unavailable: configure ISA_WEB_AGENT_WORKSPACE and ISA_WEB_MODEL on the Rust server. Custom providers and pools do not support this loop.';updateChatProviderStatus();document.querySelector('#chatPluginsStatus').textContent=data.pluginRuntime?data.catalog.skillCount+' skills available on the Rust server.':'Plugin selection is saved, but the development preview cannot use plugins.';const included=settingsCatalog.filter(item=>item.collection==='software-engineering');let fields=null;if(included.length){const group=node('div','chat-plugin-group'),head=node('div','chat-plugin-group-head'),expand=node('button','chat-plugin-expand'),switchLabel=node('label','chat-plugin-switch'),groupSwitch=node('input'),track=node('span','chat-plugin-switch-track');expand.type='button';expand.setAttribute('aria-expanded','false');expand.setAttribute('aria-controls','softwareEngineeringFields');expand.append(node('span','chevron','›'),document.createTextNode('software-engineering'),node('small','',included.length+' folders'));groupSwitch.type='checkbox';groupSwitch.id='softwareEngineeringSwitch';groupSwitch.setAttribute('role','switch');groupSwitch.setAttribute('aria-label','Enable software-engineering plugin');groupSwitch.checked=settings.softwareEngineeringEnabled!==false;track.setAttribute('aria-hidden','true');switchLabel.append(groupSwitch,track);head.append(expand,switchLabel);fields=node('fieldset','chat-plugin-fields');fields.id='softwareEngineeringFields';fields.dataset.group='software-engineering';fields.setAttribute('aria-label','Software engineering plugin folders');fields.hidden=true;expand.onclick=()=>{fields.hidden=!fields.hidden;expand.setAttribute('aria-expanded',String(!fields.hidden))};groupSwitch.addEventListener('change',updatePluginAvailability);group.append(head,fields);chatPluginList.append(group)}for(const plugin of settingsCatalog){const row=node('label','chat-plugin-row'),input=node('input');input.type='checkbox';input.value=plugin.id;input.dataset.pluginId=plugin.id;input.checked=!Array.isArray(settings.enabledPlugins)||settings.enabledPlugins.includes(plugin.id);const copy=node('span');copy.append(node('strong','',plugin.id),node('small','',plugin.count+' skills'));row.append(input,copy);(plugin.collection==='software-engineering'?fields:chatPluginList).append(row)}updatePluginAvailability()}catch(error){if(request!==settingsRequest||!chatSettingsDialog.open)return;providerStatusKnown=false;updateChatProviderStatus();document.querySelector('#serverCapabilityStatus').textContent='Server capabilities unavailable. A static preview does not provide the Rust backend.';document.querySelector('#chatPluginsStatus').textContent='Plugin catalog unavailable; existing selections will be preserved.';chatProvider.querySelector('[value="ollama"]').disabled=false}};
chatSettingsDialog.addEventListener('cancel',()=>{settingsRequest++});document.querySelector('#closeChatSettings').onclick=closeChatSettings;document.querySelector('#cancelChatSettings').onclick=closeChatSettings;
chatSettingsForm.onsubmit=event=>{event.preventDefault();const current=chats.find(chat=>chat.id===activeChatId),previous=current?.settings||draftChatSettings,settings={approvalMode:Object.hasOwn(approvalModes,previous.approvalMode)?previous.approvalMode:'always',provider:chatProvider.value,agentLoop:chatAgentLoop.checked,toolLoopOnSend:chatToolLoop.checked,poolSymbolQuery:poolSymbolQuery.value.trim(),poolSymbolMode:poolSymbolMode.value==='callers'?'callers':'declarations',pluginsMode:chatPluginsMode.checked?'auto':'off',enabledPlugins:settingsCatalog.length?(chatPluginList.querySelectorAll('input[data-plugin-id]:checked').length===settingsCatalog.length?null:Array.from(chatPluginList.querySelectorAll('input[data-plugin-id]:checked'),input=>input.value)):(previous.enabledPlugins??null),softwareEngineeringEnabled:chatPluginList.querySelector('#softwareEngineeringSwitch')?.checked??(previous.softwareEngineeringEnabled!==false)};if(settings.toolLoopOnSend&&(settings.agentLoop||!customProviders.some(p=>p.id===settings.provider))){isaNotifications.show('Tool loop on send requires one selected custom provider and cannot be combined with the read-only agent loop.');return}if(new TextEncoder().encode(settings.poolSymbolQuery).length>100||/\p{Cc}/u.test(settings.poolSymbolQuery)){isaNotifications.show('Symbol query must be at most 100 UTF-8 bytes without control characters.');return}if(current){if(!saveChats(chats.map(chat=>chat.id===current.id?{...chat,settings}:chat)))return}else draftChatSettings=settings;updateRoutingLabel();closeChatSettings();isaNotifications.show('Chat settings saved for future messages.')};
renderActivity();renderRecent();const restoredChat=sessionStorage.getItem('isa.activeChat');if(restoredChat&&chats.some(chat=>chat.id===restoredChat))openChat(restoredChat);else setActiveSection('newTask');

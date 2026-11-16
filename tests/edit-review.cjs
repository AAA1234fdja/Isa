const assert=require('node:assert/strict'),fs=require('node:fs'),{TextEncoder,TextDecoder}=require('node:util');
const {JSDOM}=require(process.env.JSDOM_PATH||'jsdom');
(async()=>{
 let calls=[],pending;
 const dom=new JSDOM(fs.readFileSync('index.html','utf8'),{url:'http://localhost:4174',runScripts:'outside-only',pretendToBeVisual:true,beforeParse(w){
  w.localStorage.setItem('isa.chats.v1',JSON.stringify([{id:'test',title:'Review test',messages:[{role:'assistant',text:'Suggested edit:\n```text\nFile: a\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n```',at:'2026-01-01T00:00:00Z'}]}]));
  w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;w.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});w.ResizeObserver=class{observe(){}disconnect(){}};w.HTMLElement.prototype.scrollIntoView=function(){};w.HTMLDialogElement.prototype.showModal=function(){this.open=true};w.HTMLDialogElement.prototype.close=function(){this.open=false};
  w.fetch=(url,options)=>{calls.push({url,options});return new Promise(resolve=>{pending=resolve})};
 }});
 const w=dom.window;for(const s of w.document.querySelectorAll('script'))if(!s.type||s.type==='text/javascript')w.eval(s.textContent);
 calls=[];w.openChat('test');w.document.querySelector('.edit-review-open').click();assert.equal(w.document.querySelector('.edit-review textarea').value,'File: a\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE');assert.match(w.document.querySelector('.edit-review [role=status]').textContent,/extracted/);
 assert.equal(w.extractReviewProposal('File: a\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\nFile: b\n<<<<<<< SEARCH\nx\n=======\ny\n>>>>>>> REPLACE'),null);
 assert.equal(w.extractReviewProposal('explanation\nFile: a\n<<<<<<< SEARCH\nold\n\n=======\nnew\n\n>>>>>>> REPLACE\nend'),'File: a\n<<<<<<< SEARCH\nold\n\n=======\nnew\n\n>>>>>>> REPLACE');assert.equal(w.extractReviewProposal('File: a\n<<<<<<< SEARCH\nold'),null);
 let d=w.document.querySelector('.edit-review'),button=[...d.querySelectorAll('button')].find(b=>b.textContent==='Check proposal');assert.equal(calls.length,0);button.click();assert.equal(calls[0].url,'/api/edit-preview');assert.deepEqual(Object.keys(JSON.parse(calls[0].options.body)),['proposal']);
 pending({ok:true,json:async()=>({readOnly:true,file:'a',line:1,search:'<img src=x onerror=alert(1)>',replacement:'<script>bad</script>'})});await new Promise(r=>setTimeout(r,10));assert.equal(d.querySelectorAll('.edit-review-output img,.edit-review-output script').length,0);assert.match(d.querySelector('.edit-review-output').textContent,/<script>bad<\/script>/);
 const toggle=d.querySelector('.edit-review-output input');toggle.checked=true;toggle.dispatchEvent(new w.Event('change'));assert.match(d.querySelector('.review-search').textContent,/·/);toggle.checked=false;toggle.dispatchEvent(new w.Event('change'));assert.equal(d.querySelector('.review-search').textContent,'<img src=x onerror=alert(1)>');
 button.click();d.querySelector('textarea').value='changed';d.querySelector('textarea').dispatchEvent(new w.Event('input'));pending({ok:true,json:async()=>({readOnly:true,file:'a',line:1,search:'old',replacement:'new'})});await new Promise(r=>setTimeout(r,10));assert.equal(d.querySelector('.edit-review-output').textContent,'');assert.match(d.querySelector('[role=status]').textContent,/changed/);
 button.click();pending({ok:false,json:async()=>({error:'No match'})});await new Promise(r=>setTimeout(r,10));assert.match(d.querySelector('[role=status]').textContent,/No match/);d.querySelector('button').click();assert.equal(w.document.querySelector('.edit-review'),null);w.close();console.log('Read-only proposal review UI passed');
})().catch(e=>{console.error(e);process.exit(1)});

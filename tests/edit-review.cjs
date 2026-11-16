const assert=require('node:assert/strict'),fs=require('node:fs'),{TextEncoder,TextDecoder}=require('node:util');
const {JSDOM}=require(process.env.JSDOM_PATH||'jsdom');
(async()=>{
 let calls=[],pending;
 const dom=new JSDOM(fs.readFileSync('index.html','utf8'),{url:'http://localhost:4174',runScripts:'outside-only',pretendToBeVisual:true,beforeParse(w){
  w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;w.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});w.ResizeObserver=class{observe(){}disconnect(){}};w.HTMLElement.prototype.scrollIntoView=function(){};w.HTMLDialogElement.prototype.showModal=function(){this.open=true};w.HTMLDialogElement.prototype.close=function(){this.open=false};
  w.fetch=(url,options)=>{calls.push({url,options});return new Promise(resolve=>{pending=resolve})};
 }});
 const w=dom.window;for(const s of w.document.querySelectorAll('script'))if(!s.type||s.type==='text/javascript')w.eval(s.textContent);
 calls=[];w.openEditReview('File: a\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE');
 let d=w.document.querySelector('.edit-review'),button=[...d.querySelectorAll('button')].find(b=>b.textContent==='Check proposal');assert.equal(calls.length,0);button.click();assert.equal(calls[0].url,'/api/edit-preview');assert.deepEqual(Object.keys(JSON.parse(calls[0].options.body)),['proposal']);
 pending({ok:true,json:async()=>({readOnly:true,file:'a',line:1,search:'<img src=x onerror=alert(1)>',replacement:'<script>bad</script>'})});await new Promise(r=>setTimeout(r,10));assert.equal(d.querySelectorAll('.edit-review-output img,.edit-review-output script').length,0);assert.match(d.querySelector('.edit-review-output').textContent,/<script>bad<\/script>/);
 button.click();d.querySelector('textarea').value='changed';d.querySelector('textarea').dispatchEvent(new w.Event('input'));pending({ok:true,json:async()=>({readOnly:true,file:'a',line:1,search:'old',replacement:'new'})});await new Promise(r=>setTimeout(r,10));assert.equal(d.querySelector('.edit-review-output').textContent,'');assert.match(d.querySelector('[role=status]').textContent,/changed/);
 button.click();pending({ok:false,json:async()=>({error:'No match'})});await new Promise(r=>setTimeout(r,10));assert.match(d.querySelector('[role=status]').textContent,/No match/);d.querySelector('button').click();assert.equal(w.document.querySelector('.edit-review'),null);w.close();console.log('Read-only proposal review UI passed');
})().catch(e=>{console.error(e);process.exit(1)});

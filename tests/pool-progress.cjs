const assert=require('node:assert/strict'),fs=require('fs'),{TextEncoder,TextDecoder}=require('node:util'),{JSDOM}=require(process.env.JSDOM_PATH||'./ui/node_modules/jsdom');
(async()=>{
 let requests=0,resolveProvider;const now='2026-01-01T00:00:00.000Z';
 const dom=new JSDOM(require('./interface-fixture.cjs')(),{url:'http://localhost:4174',runScripts:'outside-only',pretendToBeVisual:true,beforeParse(w){
  w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;w.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});w.ResizeObserver=class{observe(){}disconnect(){}};w.HTMLElement.prototype.scrollIntoView=function(){};w.HTMLDialogElement.prototype.showModal=function(){this.open=true};w.HTMLDialogElement.prototype.close=function(){this.open=false};
  w.localStorage.setItem('isa.customProviders.v1',JSON.stringify([{id:'custom-a',name:'A',model:'m',url:'https://a.example.com/chat'}]));
  w.localStorage.setItem('isa.agentPools.v1',JSON.stringify([{id:'p',name:'Team',members:['custom-a'],edges:[],graph:{nodes:[{id:'a',providerId:'custom-a',type:'agent'}],edges:[]}}]));
  w.localStorage.setItem('isa.chats.v1',JSON.stringify([{id:'a',title:'A',createdAt:now,messages:[],settings:{provider:'pool:p'}},{id:'b',title:'B',createdAt:now,messages:[],settings:{provider:'auto'}}]));
  w.fetch=(url,options)=>{if(!String(url).startsWith('https://a.example.com'))return Promise.resolve({ok:true,json:async()=>({catalog:{plugins:[]}})});requests++;return new Promise((resolve,reject)=>{resolveProvider=resolve;options.signal.addEventListener('abort',()=>reject(new w.DOMException('Stopped','AbortError')),{once:true})})};
 }});
 const w=dom.window;for(const script of w.document.querySelectorAll('script'))if(!script.type||script.type==='text/javascript')w.eval(script.textContent);
 const tick=()=>new Promise(r=>setTimeout(r,20)),status=()=>w.document.querySelector('.pending-provider-status')?.textContent;
 w.eval("openChat('a')");w.document.querySelector('#prompt').value='hello';w.document.querySelector('#send').click();await tick();const dialog=w.document.querySelector('.routing-consent');assert(dialog);[...dialog.querySelectorAll('button')].find(b=>b.textContent==='Send').click();await tick();
assert.equal(requests,1);assert.match(status(),/Requesting · A · attempt 1 of 3/);assert.match(status(),/1 pool request/);
 w.eval("openChat('b')");assert.doesNotMatch(status(),/Requesting/);w.eval("openChat('a')");assert.match(status(),/Requesting · A/);
 resolveProvider({ok:true,status:200,json:async()=>({choices:[{message:{content:'completed answer'}}]})});await tick();assert.match(w.document.querySelector('#thread').textContent,/completed answer/);assert.equal(status(),'Messages are saved locally.');
 w.document.querySelector('#prompt').value='again';w.document.querySelector('#send').click();await tick();[...w.document.querySelector('.routing-consent').querySelectorAll('button')].find(b=>b.textContent==='Send').click();await tick();assert.equal(requests,2);assert.match(status(),/Requesting/);w.document.querySelector('#send').click();await tick();assert.equal(status(),'Messages are saved locally.');assert.match(w.document.querySelector('#thread').textContent,/Request stopped/);
 w.close();console.log('Pool live status, chat isolation, completion and stop passed');
})().catch(e=>{console.error(e);process.exit(1)});

const {JSDOM}=require(process.env.JSDOM_PATH||'./ui/node_modules/jsdom');
const fs=require('fs'),assert=require('assert');
const html=require('./interface-fixture.cjs')();
const now='2026-01-01T00:00:00.000Z';
const dom=new JSDOM(html,{url:'http://localhost:4174',runScripts:'outside-only',pretendToBeVisual:true,beforeParse(w){
  w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;
  w.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});
  w.fetch=async()=>({ok:true,json:async()=>({catalog:{plugins:[]}}),arrayBuffer:async()=>new ArrayBuffer(0)});
  w.ResizeObserver=class{observe(){}disconnect(){}};w.HTMLElement.prototype.scrollIntoView=function(){};
  w.HTMLDialogElement.prototype.showModal=function(){this.open=true};w.HTMLDialogElement.prototype.close=function(){this.open=false;this.dispatchEvent(new w.Event('close'))};
  // Old global unrestricted preference must not silently grant new chats authority.
  w.localStorage.setItem('isa.approvalMode','free');
  w.localStorage.setItem('isa.chats.v1', JSON.stringify([{id:'a',title:'A',createdAt:now,messages:[],settings:{provider:'auto'}},{id:'b',title:'B',createdAt:now,messages:[],settings:{provider:'auto',approvalMode:'invalid'}}]));
}});
const w=dom.window;for(const script of w.document.querySelectorAll('script'))if(!script.type||script.type==='text/javascript')w.eval(script.textContent);
const label=()=>w.document.querySelector('#approvalModeLabel').textContent;
const choose=mode=>{w.document.querySelector('#approvalModeButton').click();w.document.querySelector(`[data-approval-mode="${mode}"]`).click()};
assert.equal(label(),'Ask every time');
w.eval("openChat('a')");
assert.equal(label(),'Ask every time');choose('risk');assert.equal(label(),'Ask occasionally');
assert.equal(JSON.parse(w.localStorage.getItem('isa.chats.v1')).find(c=>c.id==='a').settings.approvalMode,'risk');
w.eval("openChat('b')");assert.equal(label(),'Ask every time');choose('free');
assert(w.document.querySelector('#unrestrictedDialog').open);w.document.querySelector('#cancelUnrestricted').click();assert.equal(label(),'Ask every time');
choose('free');w.document.querySelector('#confirmUnrestricted').click();assert.equal(label(),'Never ask');
w.eval("openChat('a')");assert.equal(label(),'Ask occasionally');
// An open confirmation cannot authorize a different chat or a replaced draft.
choose('free');w.eval("openChat('b')");w.document.querySelector('#confirmUnrestricted').click();assert.equal(label(),'Never ask');
assert.equal(JSON.parse(w.localStorage.getItem('isa.chats.v1')).find(c=>c.id==='a').settings.approvalMode,'risk');
w.eval('newChat()');assert.equal(label(),'Ask every time');choose('risk');assert.equal(label(),'Ask occasionally');
w.eval('newChat()');assert.equal(label(),'Ask every time');
choose('free');w.eval('newChat()');w.document.querySelector('#confirmUnrestricted').click();assert.equal(label(),'Ask every time');
// Saving unrelated chat settings must not reset the existing policy.
w.eval("openChat('a')");w.document.querySelector('#more').click();
setTimeout(()=>{w.document.querySelector('#poolSymbolQuery').value='example';w.document.querySelector('#chatSettingsForm').dispatchEvent(new w.Event('submit',{bubbles:true,cancelable:true}));assert.equal(JSON.parse(w.localStorage.getItem('isa.chats.v1')).find(c=>c.id==='a').settings.approvalMode,'risk');assert.equal(JSON.parse(w.localStorage.getItem('isa.chats.v1')).find(c=>c.id==='a').settings.poolSymbolQuery,'example');console.log('Per-chat supervision preferences, safe defaults and confirmation binding passed');w.close()},0);

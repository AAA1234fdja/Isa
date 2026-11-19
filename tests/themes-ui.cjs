const assert=require('node:assert/strict');
const {JSDOM}=require(process.env.JSDOM_PATH||'./ui/node_modules/jsdom');
const html=require('./interface-fixture.cjs')();
function setup(saved={}){
  const dom=new JSDOM(html,{url:'http://127.0.0.1:4174',runScripts:'outside-only',pretendToBeVisual:true,beforeParse(w){
    w.matchMedia=()=>({matches:false,addEventListener(){},removeEventListener(){}});w.ResizeObserver=class{observe(){}disconnect(){}};
    w.HTMLElement.prototype.scrollIntoView=function(){};w.HTMLDialogElement.prototype.showModal=function(){this.open=true};w.HTMLDialogElement.prototype.close=function(){this.open=false};
    for(const [key,value] of Object.entries(saved))w.localStorage.setItem(key,value);
    w.fetch=async()=>({ok:true,json:async()=>({catalog:{plugins:[]}})});
  }});
  const w=dom.window;for(const script of w.document.querySelectorAll('script'))if(!script.type||script.type==='text/javascript')w.eval(script.textContent);
  w.document.querySelector('[data-name="Settings"]').click();return dom;
}
let dom=setup(),w=dom.window,d=w.document;
assert.equal(d.querySelectorAll('.theme-grid .settings-option').length,33);
assert.equal(d.querySelector('.theme-count').textContent,'33 / 33');
let search=d.querySelector('[aria-label="Search themes"]');search.value='ocean';search.dispatchEvent(new w.Event('input',{bubbles:true}));
assert.equal(d.querySelector('.theme-count').textContent,'1 / 33');
const ocean=d.querySelector('.theme-grid button');assert.equal(ocean.querySelector('strong').textContent,'Ocean');ocean.click();
assert.equal(w.localStorage.getItem('isa.accent'),'ocean');assert.equal(d.documentElement.dataset.palette,'ocean');
assert.equal(d.querySelector('[aria-label="Search themes"]').value,'ocean');assert.equal(d.activeElement.querySelector('strong').textContent,'Ocean');
assert.equal(d.querySelector('.theme-grid .selected').getAttribute('aria-pressed'),'true');
const dark=[...d.querySelectorAll('.settings-option')].find(b=>b.querySelector('strong')?.textContent==='Dark');dark.click();
assert.equal(d.documentElement.dataset.theme,'dark');assert.ok(d.documentElement.style.getPropertyValue('--palette-main').includes('14%'));
search=d.querySelector('[aria-label="Search themes"]');search.value='no such palette';search.dispatchEvent(new w.Event('input',{bubbles:true}));assert.match(d.querySelector('.theme-empty').textContent,/No themes found/);
const saved={'isa.accent':w.localStorage.getItem('isa.accent'),'isa.appearance':w.localStorage.getItem('isa.appearance')};dom.window.close();
dom=setup(saved);w=dom.window;d=w.document;assert.equal(d.documentElement.dataset.palette,'ocean');assert.equal(d.documentElement.dataset.theme,'dark');assert.equal([...d.querySelectorAll('.theme-grid .selected strong')][0].textContent,'Ocean');
w.Storage.prototype.setItem=function(){throw Error('denied')};const cherry=[...d.querySelectorAll('.theme-grid button')].find(b=>b.querySelector('strong')?.textContent==='Cherry');cherry.click();assert.equal(d.documentElement.dataset.palette,'ocean');assert.match(d.querySelector('#toast').textContent,/Could not save/);
dom.window.close();console.log('Theme catalog search, selection, persistence, dark mode and failed writes passed');

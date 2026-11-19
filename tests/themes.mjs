import assert from 'node:assert/strict';
import {themes, paletteFor} from '../frontend/src/themes.mjs';
import {createAppearance} from '../frontend/src/appearance.mjs';
import {createRequire} from 'node:module';
const {JSDOM}=createRequire(import.meta.url)(process.env.JSDOM_PATH || './ui/node_modules/jsdom');
assert.equal(themes.length,33);assert.equal(new Set(themes.map(t=>t.id)).size,33);
assert.equal(themes.slice(0,3).map(t=>t.id).join(','),'forest,moss,sage');
function luminance(hsl) {
  const [,h,s,l]=/^hsl\((\d+), (\d+)%, (\d+)%\)$/.exec(hsl).map(Number);
  const a=s/100*Math.min(l/100,1-l/100),f=n=>{const k=(n+h/30)%12;return l/100-a*Math.max(-1,Math.min(k-3,9-k,1))};
  return [f(0),f(8),f(4)].map(v=>v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4).reduce((sum,v,i)=>sum+v*[0.2126,0.7152,0.0722][i],0);
}
const ratio=(a,b)=>{const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05)};
for(const t of themes.slice(3)){
  assert.ok((1.05/(luminance(t.color)+.05))>=4.5, t.name+' white button text');
  for(const dark of [false,true]){
    const palette=paletteFor(t,dark);
    assert.ok(ratio(palette['--palette-ink'],palette['--palette-main'])>=4.5,t.name+' main text');
    assert.ok(ratio(palette['--palette-ink'],palette['--palette-active'])>=4.5,t.name+' active text');
  }
}
const dom=new JSDOM('<!doctype html>',{url:'https://example.com'}),w=dom.window;
let change,media={matches:false,addEventListener(type,callback){change=callback}};w.matchMedia=()=>media;
w.localStorage.setItem('isa.accent','ocean');w.localStorage.setItem('isa.appearance','system');
const controller=createAppearance({window:w,document:w.document});
assert.equal(controller.accent,'ocean');assert.equal(w.document.documentElement.dataset.theme,'light');
const before=w.document.documentElement.style.getPropertyValue('--palette-main');media.matches=true;change();
assert.equal(w.document.documentElement.dataset.theme,'dark');assert.notEqual(w.document.documentElement.style.getPropertyValue('--palette-main'),before);
controller.setAccent('forest');assert.equal(w.document.documentElement.style.getPropertyValue('--palette-main'),'');assert.equal(w.document.documentElement.style.getPropertyValue('--green'),'#315f49');
assert.throws(()=>controller.setAccent('__proto__'));dom.window.close();
console.log('All 33 themes are distinct and new palettes meet tested text contrast thresholds');

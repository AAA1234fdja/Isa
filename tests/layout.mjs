import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createRequire} from 'node:module';
import {mountLayout} from '../frontend/src/layout.mjs';
const require=createRequire(import.meta.url);
const {JSDOM}=require(process.env.JSDOM_PATH || './ui/node_modules/jsdom');
const html=fs.readFileSync('index.html','utf8');
const source=new JSDOM(html,{runScripts:'outside-only'}), bundled=new JSDOM(html,{runScripts:'outside-only'});
mountLayout(source.window.document);
bundled.window.eval(fs.readFileSync('assets/layout.js','utf8'));
function serialize(doc){
  function walk(node){if(node.nodeType===3)return node.nodeValue;return [node.namespaceURI,node.localName,[...node.attributes].map(a=>[a.name,a.value]),[...node.childNodes].map(walk)];}
  return [...doc.body.children].filter(n=>n.tagName!=='SCRIPT'&&n.tagName!=='NOSCRIPT').map(walk);
}
assert.deepEqual(serialize(bundled.window.document),serialize(source.window.document));
const d=source.window.document;
assert.equal(d.querySelector('#app-root'),null);
for(const selector of ['.app','#prompt','#thread','#sidebar','#projects','#hub','#projectDialog','#attachmentDialog','#chatSettingsForm','#toast']) assert.ok(d.querySelector(selector),selector);
assert.equal(d.querySelector('#prompt').getAttribute('aria-label'),'Message ISA');
assert.equal(d.querySelector('#unrestrictedDialog').getAttribute('aria-describedby'),'unrestrictedDescription');
assert.equal(d.querySelector('svg').namespaceURI,'http://www.w3.org/2000/svg');
assert.equal(d.querySelector('svg path').namespaceURI,'http://www.w3.org/2000/svg');
assert.equal(d.querySelector('svg').getAttribute('viewBox'),'0 0 20 20');
assert.equal(d.querySelector('#projectDescription').maxLength,240);
assert.throws(()=>mountLayout(d),/root is missing/);
source.window.close();bundled.window.close();
console.log('Layout module and bundle mount the same accessible DOM with SVG namespaces');

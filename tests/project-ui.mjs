import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {createProjectStore} from '../frontend/src/project-store.mjs';
import {createProjectUI} from '../frontend/src/project-ui.mjs';
const require=createRequire(import.meta.url);
const {JSDOM}=require(process.env.JSDOM_PATH || './ui/node_modules/jsdom');
function setup() {
  const dom=new JSDOM('<div id="sidebarProjects"></div><div id="recentChats"></div>',{url:'https://example.com'});
  const {window}=dom, document=window.document;
  const store=createProjectStore({getStorage:()=>window.localStorage});
  store.load();store.add({id:'p',name:'Project',description:'',chatIds:[]});
  let notices=[];
  const ui=createProjectUI({document,projectsSection:{hidden:true},projectStore:store,chats:[{id:'c',title:'Chat',messages:[]}],isaNotifications:{show:m=>notices.push(m)},renderRecent(){},node(tag,className,text){const element=document.createElement(tag);if(className)element.className=className;if(text!==undefined)element.textContent=text;return element}});
  ui.renderSidebarProjects();
  return {dom,window,document,store,ui,notices};
}
const a=setup(), b=setup();
const drag=a.document.createElement('button');a.document.body.append(drag);a.ui.enableChatDrag(drag,{id:'c'});
const transfer={types:[],data:{},setData(type,value){this.types.push(type);this.data[type]=value},getData(type){return this.data[type]}};
function emit(w,element,type){const event=new w.Event(type,{bubbles:true,cancelable:true});Object.defineProperty(event,'dataTransfer',{value:transfer});element.dispatchEvent(event)}
emit(a.window,drag,'dragstart');
emit(b.window,b.document.querySelector('.sidebar-project'),'drop');
assert.deepEqual(b.store.list()[0].chatIds,[],'drag context is not shared across instances');
emit(a.window,a.document.querySelector('.sidebar-project'),'drop');
assert.deepEqual(a.store.list()[0].chatIds,['c']);
assert.deepEqual(b.store.list()[0].chatIds,[]);
assert.equal(JSON.parse(a.window.localStorage.getItem('isa.preview.projects'))[0].chatIds[0],'c');
assert.ok(a.notices.some(message=>message.includes('Conversation added')));
a.dom.window.close();b.dom.window.close();
console.log('Project UI module isolates drag state and updates only its injected store');

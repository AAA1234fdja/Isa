const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const net = require('node:net');
const { JSDOM, VirtualConsole } = require(process.env.JSDOM_PATH || './ui/node_modules/jsdom');

(async () => {
  const probe = net.createServer();
  await new Promise((resolve, reject) => probe.once('error', reject).listen(0, '127.0.0.1', resolve));
  const port = probe.address().port;
  await new Promise(resolve => probe.close(resolve));
  const child = spawn(process.execPath, ['dev/preview.mjs'], { env: { ...process.env, PORT: String(port) }, stdio: ['ignore', 'pipe', 'pipe'] });
  let dom;
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('preview startup timeout')), 5000);
      child.once('exit', () => { clearTimeout(timer); reject(Error('preview exited')); });
      child.stdout.on('data', chunk => { if (String(chunk).includes('listening')) { clearTimeout(timer); resolve(); } });
    });
    const errors = [];
    const calls = [];
    const virtualConsole = new VirtualConsole();
    virtualConsole.on('jsdomError', error => { if (error.type !== 'css parsing') errors.push(error.message); });
    dom = await JSDOM.fromURL(`http://127.0.0.1:${port}/`, {
      resources: 'usable', runScripts: 'dangerously', pretendToBeVisual: true, virtualConsole,
      beforeParse(w) {
        w.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
        w.ResizeObserver = class { observe() {} disconnect() {} };
        w.fetch = async (url, options = {}) => {
          if (!String(url).startsWith('/api/exec/')) return {ok:false,json:async()=>({})};
          const data = JSON.parse(options.body); calls.push({url, data});
          let response;
          if (url === '/api/exec/policy') response = {mode:data.mode,executionProfile:'host',workspace:'/workspace'};
          else if (url === '/api/exec/prepare') response = {mode:'always',executionProfile:'host',id:'a'.repeat(48),command:data.command,workspace:'/workspace',requiresApproval:true};
          else if (url === '/api/exec/run') response = {result:{exitCode:0,timedOut:false,stdout:'mock output',stderr:'',truncated:false,pipesIncomplete:false}};
          else response = {cancelled:true};
          return {ok:true,json:async()=>response};
        };
        w.TextEncoder = TextEncoder; w.TextDecoder = TextDecoder;
        w.HTMLElement.prototype.scrollIntoView = function () {};
        w.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
        w.HTMLDialogElement.prototype.close = function () { this.open = false; };
        w.localStorage.setItem('isa.chats.v1', JSON.stringify([{id:'real-load',title:'Test',messages:[],settings:{approvalMode:'always'}}]));
      },
    });
    await new Promise((resolve, reject) => {
      if (dom.window.document.readyState === 'complete') return resolve();
      const timer = setTimeout(() => reject(Error('page load timeout')), 5000);
      dom.window.addEventListener('load', () => { clearTimeout(timer); resolve(); }, { once: true });
    });
    assert.deepEqual(errors, []);
    assert.equal(dom.window.document.querySelector('link[rel="stylesheet"]').sheet != null, true);
    assert.equal(dom.window.getComputedStyle(dom.window.document.body).margin, '0px');
    const optional = dom.window.document.querySelector('.optional-label');
    assert.equal(dom.window.getComputedStyle(optional).fontWeight, '400');
    assert.equal(dom.window.getComputedStyle(optional).color, 'rgb(161, 171, 160)');
    assert.equal(typeof dom.window.eval('openChat'), 'function');
    assert.equal(typeof dom.window.eval('openLocalCommand'), 'function');
    assert.equal(typeof dom.window.eval('openToolLoop'), 'function');
    assert.equal(dom.window.eval("attachmentKind({type: 'image/png'}, 'image.png')"), 'image');
    dom.window.eval('clearAttachments()');
    dom.window.document.querySelector('#menu').click();
    assert.equal(dom.window.document.querySelector('#menu').getAttribute('aria-expanded'), 'true');
    dom.window.document.querySelector('[data-name="Settings"]').click();
    assert.equal(dom.window.document.querySelector('#menu').getAttribute('aria-expanded'), 'false');
    const dark = [...dom.window.document.querySelectorAll('.settings-option')].find(button => button.querySelector('strong')?.textContent === 'Dark');
    assert.ok(dark, 'appearance settings are usable');
    dark.click();
    assert.equal(dom.window.document.documentElement.dataset.theme, 'dark');
    assert.equal(dom.window.localStorage.getItem('isa.appearance'), 'dark');
    const moss = [...dom.window.document.querySelectorAll('.settings-option')].find(button => button.querySelector('strong')?.textContent === 'Moss');
    assert.ok(moss); moss.click();
    assert.equal(dom.window.document.documentElement.dataset.palette, 'moss');
    assert.equal(dom.window.localStorage.getItem('isa.accent'), 'moss');
    const themeSearch = dom.window.document.querySelector('[aria-label="Search themes"]');
    themeSearch.value = 'ocean'; themeSearch.dispatchEvent(new dom.window.Event('input', {bubbles:true}));
    const ocean = dom.window.document.querySelector('.theme-grid .settings-option');
    assert.equal(ocean.querySelector('strong').textContent, 'Ocean'); ocean.click();
    assert.equal(dom.window.document.documentElement.dataset.palette, 'ocean');
    assert.equal(dom.window.localStorage.getItem('isa.accent'), 'ocean');
    assert.ok(dom.window.document.documentElement.style.getPropertyValue('--palette-main'));
    const slider = dom.window.document.querySelector('#scaleSetting');
    slider.value = '125'; slider.dispatchEvent(new dom.window.Event('input', {bubbles:true}));
    assert.equal(dom.window.localStorage.getItem('isa.scale'), '1.25');
    assert.equal(dom.window.document.documentElement.style.getPropertyValue('--app-scale'), '1.25');
    const motion = dom.window.document.querySelector('.settings-toggle input[type=checkbox]');
    motion.checked = true; motion.dispatchEvent(new dom.window.Event('change', {bubbles:true}));
    assert.equal(dom.window.document.documentElement.dataset.reduceMotion, 'true');
    assert.equal(dom.window.localStorage.getItem('isa.reduceMotion'), 'true');
    dom.window.document.querySelector('#projectsNav').click();
    assert.equal(dom.window.document.querySelector('#projects').hidden, false);
    assert.match(dom.window.document.querySelector('#projects').textContent, /Projects/);
    dom.window.document.querySelector('[data-name="Agents"]').click();
    assert.match(dom.window.document.querySelector('#hub').textContent, /Agents/);
    dom.window.eval("openChat('real-load')");
    dom.window.document.querySelector('#runCommandButton').click();
    const dialog = [...dom.window.document.querySelectorAll('dialog')].find(item => item.open && item.textContent.includes('Run a local command'));
    assert.ok(dialog, 'supervised dialog must access shared chat state across actual scripts');
    dialog.querySelector('input[type=password]').value = 'test-token';
    dialog.querySelector('textarea').value = 'echo test';
    dialog.querySelector('form').dispatchEvent(new dom.window.Event('submit', {bubbles:true,cancelable:true}));
    await new Promise(resolve => setTimeout(resolve, 20));
    const approve = [...dialog.querySelectorAll('button')].find(button => button.textContent === 'Approve and run');
    assert.ok(approve, 'approval is required before mocked execution');
    assert.equal(calls.filter(call => call.url === '/api/exec/run').length, 0);
    approve.click();
    await new Promise(resolve => setTimeout(resolve, 20));
    assert.equal(calls.filter(call => call.url === '/api/exec/run').length, 1);
    assert.match(dom.window.document.querySelector('#thread').textContent, /mock output/);
    console.log('Preview loads assets, appearance, routing and mocked supervised approval across scripts');
  } finally {
    if (dom) dom.window.close();
    child.kill();
    if (child.exitCode === null && child.signalCode === null) await new Promise(resolve => child.once('exit', resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });

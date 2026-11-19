const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');

(async () => {
  const probe = net.createServer();
  await new Promise((resolve, reject) => probe.once('error', reject).listen(0, '127.0.0.1', resolve));
  const port = probe.address().port;
  await new Promise(resolve => probe.close(resolve));
  const child = spawn(process.execPath, ['dev/preview.mjs'], { env: { ...process.env, PORT: String(port) }, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('preview startup timeout')), 5000);
      child.once('exit', () => { clearTimeout(timer); reject(Error('preview exited')); });
      child.stdout.on('data', chunk => { if (String(chunk).includes('listening')) { clearTimeout(timer); resolve(); } });
    });
    const origin = `http://127.0.0.1:${port}`;
    const html = await (await fetch(origin + '/')).text();
    assert.match(html, /<link rel="stylesheet" href="\/assets\/interface\.css">/);
    const response = await fetch(origin + '/assets/interface.css');
    assert.equal(response.status, 200);
    assert.match(response.headers.get('content-type'), /^text\/css/);
    assert.equal(await response.text(), fs.readFileSync('assets/interface.css', 'utf8'));
    const head = await fetch(origin + '/assets/interface.css', { method: 'HEAD' });
    assert.equal(head.status, 200);
    assert.match(head.headers.get('content-type'), /^text\/css/);
    assert.equal(await head.text(), '');
    const scripts = ['preview.js', 'layout.js', 'appearance.js', 'sidebar.js', 'notifications.js', 'provider-utils.js', 'conversation-utils.js', 'provider-client.js', 'pool-runner.js', 'project-store.js', 'project-ui-module.js', 'supervised-actions.js', 'attachments.js', 'projects.js', 'settings.js', 'agents.js', 'routing.js', 'chat.js', 'interface.js'];
    assert.ok(html.includes(scripts.map(name => `<script src="/assets/${name}"></script>`).join('')));
    for (const name of scripts) {
      const resource = await fetch(origin + '/assets/' + name);
      assert.equal(resource.status, 200, name);
      assert.match(resource.headers.get('content-type'), /^text\/javascript/);
      assert.equal(await resource.text(), fs.readFileSync('assets/' + name, 'utf8'));
      const scriptHead = await fetch(origin + '/assets/' + name, { method: 'HEAD' });
      assert.equal(scriptHead.status, 200, name + ' HEAD');
      assert.match(scriptHead.headers.get('content-type'), /^text\/javascript/);
      assert.equal(await scriptHead.text(), '');
    }
    const attachments = await fetch(origin + '/assets/attachments.js');
    assert.equal(attachments.status, 200);
    assert.match(attachments.headers.get('content-type'), /^text\/javascript/);
    assert.equal(await attachments.text(), fs.readFileSync('assets/attachments.js', 'utf8'));
    const actions = await fetch(origin + '/assets/supervised-actions.js');
    assert.equal(actions.status, 200);
    assert.match(actions.headers.get('content-type'), /^text\/javascript/);
    assert.equal(await actions.text(), fs.readFileSync('assets/supervised-actions.js', 'utf8'));
    const appearance = await fetch(origin + '/assets/appearance.js');
    assert.equal(appearance.status, 200);
    assert.match(appearance.headers.get('content-type'), /^text\/javascript/);
    assert.equal(await appearance.text(), fs.readFileSync('assets/appearance.js', 'utf8'));
    const previewScript = await fetch(origin + '/assets/preview.js');
    assert.equal(previewScript.status, 200);
    assert.match(previewScript.headers.get('content-type'), /^text\/javascript/);
    assert.equal(await previewScript.text(), fs.readFileSync('assets/preview.js', 'utf8'));
    const script = await fetch(origin + '/assets/interface.js');
    assert.equal(script.status, 200);
    assert.match(script.headers.get('content-type'), /^text\/javascript/);
    assert.equal(await script.text(), fs.readFileSync('assets/interface.js', 'utf8'));
    const scriptHead = await fetch(origin + '/assets/interface.js', { method: 'HEAD' });
    assert.equal(scriptHead.status, 200);
    assert.equal(await scriptHead.text(), '');
    assert.equal((await fetch(origin + '/assets/not-allowed.css')).status, 404);
    console.log('Preview serves the external stylesheet with GET and HEAD');
  } finally {
    child.kill();
    if (child.exitCode === null && child.signalCode === null) await new Promise(resolve => child.once('exit', resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });

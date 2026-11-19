const assert = require('node:assert/strict');
const fs = require('node:fs');
const {JSDOM} = require(process.env.JSDOM_PATH || './ui/node_modules/jsdom');
const html = fs.readFileSync('index.html', 'utf8');
const dom = new JSDOM(html, {url:'http://127.0.0.1:4174/', runScripts:'outside-only'});
const doc = dom.window.document;
assert.equal(doc.querySelectorAll('style, [style]').length, 0, 'HTML must not contain embedded CSS');
for (const node of doc.querySelectorAll('*')) {
  for (const attr of node.attributes) {
    assert.ok(!/^on/i.test(attr.name), `inline event handler ${attr.name}`);
    assert.ok(!/^(?:href|src|action|formaction)$/i.test(attr.name) || !/^\s*javascript:/i.test(attr.value), 'javascript URL');
  }
}
const scripts = [...doc.querySelectorAll('script')];
assert.ok(scripts.length);
for (const script of scripts) {
  assert.match(script.getAttribute('src') || '', /^\/assets\/[a-z-]+\.js$/);
  assert.equal(script.textContent.trim(), '', 'HTML must not contain embedded JavaScript');
  assert.ok(fs.existsSync('.' + script.getAttribute('src')), 'script source exists');
}
const stylesheets = [...doc.querySelectorAll('link[rel="stylesheet"]')];
assert.equal(stylesheets.length, 1);
assert.equal(stylesheets[0].getAttribute('href'), '/assets/interface.css');
assert.ok(fs.readFileSync('assets/interface.css', 'utf8').includes('.optional-label{font-weight:400;color:#a1aba0}'));
assert.deepEqual([...doc.body.children].filter(el => el.tagName !== 'SCRIPT').map(el => el.tagName), ['DIV', 'NOSCRIPT']);
assert.equal(doc.querySelector('#app-root').children.length, 0);
dom.window.eval(fs.readFileSync('assets/layout.js', 'utf8'));
assert.equal(doc.querySelector('#app-root'), null);
assert.equal(doc.querySelector('.optional-label').textContent, '(optional)');
// The Rust server must embed and serve each first-party script when started
// outside the repository. Compilation and runtime behavior are tested separately.
const rust = fs.readFileSync('src/main.rs', 'utf8');
const preview = fs.readFileSync('dev/preview.mjs', 'utf8');
for (const script of scripts) {
  const src = script.getAttribute('src');
  assert.ok(rust.includes(`"..${src}"`), `Rust embed for ${src}`);
  assert.ok(rust.includes(`"${src}"`), `Rust route for ${src}`);
  assert.ok(preview.includes(`'${src}'`), `Preview route for ${src}`);
}
dom.window.close();
console.log('HTML contains structure only; CSS and JavaScript are external and routed');

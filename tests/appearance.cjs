const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('assets/appearance.js', 'utf8');

function setup(saved = {}) {
  const style = {}, events = {}, media = {matches: false, addEventListener(type, callback) { events.media = callback; }};
  const html = {dataset: {}, style: {setProperty(name, value) { style[name] = value; }, removeProperty(name) { delete style[name]; }}};
  const window = {devicePixelRatio: 2, matchMedia: () => media, addEventListener(type, callback) { events[type] = callback; }};
  window.localStorage = {getItem(key) { return saved[key] ?? null; }};
  const context = vm.createContext({window, document: {documentElement: html}});
  vm.runInContext(source, context);
  return {context, html, window, media, style, events};
}

{
  const x = setup({'isa.appearance':'system', 'isa.accent':'sage', 'isa.scale':'1.25', 'isa.reduceMotion':'true'});
  assert.equal(x.html.dataset.theme, 'light');
  assert.equal(x.html.dataset.palette, 'sage');
  assert.equal(x.html.dataset.reduceMotion, 'true');
  assert.equal(x.style['--app-scale'], '1.25');
  x.media.matches = true; x.events.media();
  assert.equal(x.html.dataset.theme, 'dark');
  x.window.devicePixelRatio = 4; x.events.resize();
  assert.equal(x.style['--app-scale'], '0.625');
  vm.runInContext("isaAppearance.setMode('light'); isaAppearance.setAccent('moss'); isaAppearance.setReducedMotion(false); isaAppearance.setScale(1)", x.context);
  assert.equal(x.html.dataset.theme, 'light');
  assert.equal(x.html.dataset.palette, 'moss');
  assert.equal(x.html.dataset.reduceMotion, 'false');
  assert.equal(x.style['--app-scale'], '0.5');
  for (const expr of ["isaAppearance.setMode('invalid')", "isaAppearance.setAccent('__proto__')", "isaAppearance.setReducedMotion('yes')", 'isaAppearance.setScale(NaN)', 'isaAppearance.setScale(2)']) {
    assert.throws(() => vm.runInContext(expr, x.context));
  }
}
{
  const x = setup({'isa.appearance':'invalid', 'isa.accent':'bad', 'isa.scale':'0', 'isa.reduceMotion':'no'});
  assert.equal(x.html.dataset.theme, 'light');
  assert.equal(x.html.dataset.palette, 'forest');
  assert.equal(x.style['--app-scale'], '1');
}
console.log('Appearance controller isolates state and handles settings, validation and zoom');

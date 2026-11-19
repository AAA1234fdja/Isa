import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {createAppearance} from '../frontend/src/appearance.mjs';
import {createSidebar} from '../frontend/src/sidebar.mjs';
import {createNotifications} from '../frontend/src/notifications.mjs';

const require = createRequire(import.meta.url);
const {JSDOM} = require(process.env.JSDOM_PATH || './ui/node_modules/jsdom');
function setup() {
  const dom = new JSDOM('<div class="app"><aside id="sidebar"><button id="collapseSidebar"></button></aside><button id="expandSidebar"></button><button id="menu"></button><div id="toast"></div><dialog open></dialog></div>', {url:'https://example.com/'});
  const window = dom.window;
  window.matchMedia = () => ({matches:false, addEventListener(){}});
  return {dom, window, document:window.document};
}
const a = setup(), b = setup();
const appearanceA = createAppearance(a), appearanceB = createAppearance(b);
appearanceA.setAccent('sage'); appearanceA.setMode('dark'); appearanceA.setScale(1.25);
assert.equal(a.document.documentElement.dataset.theme, 'dark');
assert.equal(b.document.documentElement.dataset.theme, 'light');
assert.equal(appearanceB.accent, 'forest');
assert.equal(appearanceB.scale, 1);
const sidebarA = createSidebar(a), sidebarB = createSidebar(b);
a.document.querySelector('#menu').click(); b.document.querySelector('#menu').click(); sidebarA.close();
assert.equal(a.document.querySelector('#menu').getAttribute('aria-expanded'), 'false');
assert.equal(b.document.querySelector('#menu').getAttribute('aria-expanded'), 'true');
sidebarB.close();
const notificationsA = createNotifications(a), notificationsB = createNotifications(b);
notificationsA.reportAuditWarning({auditWarning:'metadata_incomplete'});
assert.ok(a.document.querySelector('.audit-warning'));
assert.equal(b.document.querySelector('.audit-warning'), null);
notificationsB.show('Second instance');
assert.equal(a.document.querySelector('#toast').textContent, 'Audit event history is incomplete or volatile.');
assert.equal(b.document.querySelector('#toast').textContent, 'Second instance');
a.dom.window.close(); b.dom.window.close();
console.log('ES module factories work with injected environments and independent instances');

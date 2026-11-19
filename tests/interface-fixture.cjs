// The DOM tests execute scripts manually in jsdom (outside-only). Inline the
// exact first-party asset for those tests; the real servers serve it separately.
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
module.exports = function interfaceFixture() {
  let html = fs.readFileSync(path.join(root, 'index.html'), 'utf8');
  for (const name of ['preview.js', 'layout.js', 'appearance.js', 'sidebar.js', 'notifications.js', 'provider-utils.js', 'conversation-utils.js', 'provider-client.js', 'pool-runner.js', 'project-store.js', 'project-ui-module.js', 'supervised-actions.js', 'attachments.js', 'projects.js', 'settings.js', 'agents.js', 'routing.js', 'chat.js', 'interface.js']) {
    const marker = `<script src="/assets/${name}"></script>`;
    if (!html.includes(marker) || html.indexOf(marker) !== html.lastIndexOf(marker)) throw Error(`Script marker missing or duplicated: ${name}`);
  }
  const read = name => {
    const script = fs.readFileSync(path.join(root, 'assets', name), 'utf8');
    if (/<\/script/i.test(script)) throw Error('Cannot safely inline script in test fixture');
    return script;
  };
  // Outside-only tests use separate eval calls, which do not preserve top-level
  // lexical declarations like separate browser script elements do. Combine the
  // dependent scripts in those fixtures; preview-browser tests real loading.
  html = html.replace('<script src="/assets/preview.js"></script>', () => `<script>${read('preview.js')}</script>`);
  const names = ['layout.js', 'appearance.js', 'sidebar.js', 'notifications.js', 'provider-utils.js', 'conversation-utils.js', 'provider-client.js', 'pool-runner.js', 'project-store.js', 'project-ui-module.js', 'supervised-actions.js', 'attachments.js', 'projects.js', 'settings.js', 'agents.js', 'routing.js', 'chat.js', 'interface.js'];
  const scripts = names.map(name => `<script src="/assets/${name}"></script>`).join('');
  if (!html.includes(scripts)) throw Error('Interface scripts must be adjacent and ordered');
  html = html.replace(scripts, () => `<script>${names.map(read).join('\n')}</script>`);
  return html;
};

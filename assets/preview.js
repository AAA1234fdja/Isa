
// Development version monitor. A changed asset reloads the current page.
// Failed requests leave the current interface intact and are retried later.
async function getPreviewVersion() {
  const urls = ['/index.html', '/assets/interface.css', '/assets/preview.js', '/assets/layout.js', '/assets/appearance.js', '/assets/sidebar.js', '/assets/notifications.js', '/assets/provider-utils.js', '/assets/conversation-utils.js', '/assets/provider-client.js', '/assets/pool-runner.js', '/assets/project-store.js', '/assets/project-ui-module.js', '/assets/supervised-actions.js', '/assets/attachments.js', '/assets/projects.js', '/assets/settings.js', '/assets/agents.js', '/assets/routing.js', '/assets/chat.js', '/assets/interface.js', '/assets/isa-logo-transparent.png'];
  const versions = [];
  for (const url of urls) {
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) throw new Error('Preview asset unavailable');
    const bytes = new Uint8Array(await response.arrayBuffer());
    let hash = 2166136261;
    for (const byte of bytes) hash = Math.imul(hash ^ byte, 16777619);
    versions.push(bytes.length + ':' + (hash >>> 0));
  }
  return versions.join('|');
}
let previewVersion;
async function checkPreviewVersion() {
  try {
    const current = await getPreviewVersion();
    if (previewVersion && current !== previewVersion) {
      window.location.reload();
      return;
    }
    previewVersion = current;
  } catch (_) {
    // A temporary server outage should not erase the current session.
  }
  setTimeout(checkPreviewVersion, 2000);
}
checkPreviewVersion();

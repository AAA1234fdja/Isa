// Explicit browser dependencies keep this controller independent of legacy globals.
export function createNotifications({window, document}) {
  const toast = document.querySelector('#toast');
  const auditMessages = Object.freeze({
    metadata_incomplete: 'Audit event history is incomplete or volatile.',
    encrypted_details_incomplete: 'Encrypted audit details could not be stored; no plaintext fallback.',
    both_incomplete: 'Audit event history and encrypted details are incomplete; no plaintext fallback.',
  });
  let timer;
  function show(message) {
    toast.textContent = message;
    toast.style.display = 'block';
    window.clearTimeout(timer);
    timer = window.setTimeout(() => { toast.style.display = 'none'; }, 2800);
  }
  function reportAuditWarning(data) {
    const message = Object.hasOwn(auditMessages, data?.auditWarning) ? auditMessages[data.auditWarning] : null;
    if (!message) return;
    show(message);
    for (const dialog of document.querySelectorAll('dialog[open]')) {
      let warning = dialog.querySelector('.audit-warning');
      if (!warning) {
        warning = document.createElement('p');
        warning.className = 'audit-warning';
        warning.setAttribute('role', 'alert');
        dialog.prepend(warning);
      }
      warning.textContent = message + ' The action can continue; check local audit configuration and storage.';
    }
  }
  return Object.freeze({show, reportAuditWarning});
}

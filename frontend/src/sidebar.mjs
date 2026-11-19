// Explicit browser dependencies keep this controller independent of legacy globals.
export function createSidebar({window, document}) {
  const sidebar = document.querySelector('#sidebar');
  const app = document.querySelector('.app');
  const collapse = document.querySelector('#collapseSidebar');
  const expand = document.querySelector('#expandSidebar');
  const menu = document.querySelector('#menu');
  let collapsed = false;
  try { collapsed = window.localStorage.getItem('isa.sidebar.collapsed') === 'true'; } catch (_) {}
  function render() {
    const desktop = window.matchMedia('(min-width:701px)').matches;
    app.classList.toggle('sidebar-collapsed', desktop && collapsed);
    sidebar.inert = desktop && collapsed;
    collapse.setAttribute('aria-expanded', String(!(desktop && collapsed)));
    expand.setAttribute('aria-expanded', String(!(desktop && collapsed)));
    menu.setAttribute('aria-expanded', String(sidebar.classList.contains('open')));
  }
  function persist() {
    try { window.localStorage.setItem('isa.sidebar.collapsed', String(collapsed)); } catch (_) {}
  }
  function close() { sidebar.classList.remove('open'); render(); }
  collapse.onclick = () => {
    if (window.matchMedia('(max-width:700px)').matches) { close(); menu.focus(); return; }
    collapsed = true; persist(); render(); expand.focus();
  };
  expand.onclick = () => { collapsed = false; persist(); render(); collapse.focus(); };
  menu.onclick = () => { sidebar.classList.toggle('open'); render(); };
  window.addEventListener('resize', render);
  render();
  return Object.freeze({close});
}

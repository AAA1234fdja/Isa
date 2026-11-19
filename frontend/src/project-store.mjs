// Projects are local organizational metadata. Chats remain in a separate store.
export function createProjectStore({getStorage, key = 'isa.preview.projects'}) {
  let items = [];
  function load() {
    try {
      const saved = JSON.parse(getStorage().getItem(key) || '[]');
      items = Array.isArray(saved) ? saved.filter(item => item && typeof item.id === 'string' && typeof item.name === 'string' && typeof item.description === 'string') : [];
      return true;
    } catch (_) { return false; }
  }
  function commit(next) {
    try {
      getStorage().setItem(key, JSON.stringify(next));
      items = next;
      return true;
    } catch (_) { return false; }
  }
  return Object.freeze({
    load,
    list() { return items.slice(); },
    add(project) { return commit([project, ...items]); },
    update(id, change) { return commit(items.map(project => project.id === id ? {...project, ...change} : project)); },
    remove(id) { return commit(items.filter(project => project.id !== id)); },
  });
}

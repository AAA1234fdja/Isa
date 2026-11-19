import {themes, paletteFor} from './themes.mjs';

// Explicit browser dependencies keep this controller independent of legacy globals.
export function createAppearance({window, document}) {
  const modes = ['light', 'dark', 'system'];
  const byId = new Map(themes.map(theme => [theme.id, theme]));
  let mode = 'light', accent = 'forest', reducedMotion = false, scale = 1;
  try {
    const savedMode = window.localStorage.getItem('isa.appearance');
    if (modes.includes(savedMode)) mode = savedMode;
    const savedAccent = window.localStorage.getItem('isa.accent');
    if (byId.has(savedAccent)) accent = savedAccent;
    reducedMotion = window.localStorage.getItem('isa.reduceMotion') === 'true';
    const savedScale = Number(window.localStorage.getItem('isa.scale'));
    if (savedScale >= 0.75 && savedScale <= 1.5) scale = savedScale;
  } catch (_) {}
  const initialPixelRatio = window.devicePixelRatio || 1;
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  function apply() {
    const dark = mode === 'dark' || (mode === 'system' && media.matches);
    const root = document.documentElement, theme = byId.get(accent);
    root.dataset.theme = dark ? 'dark' : 'light';
    root.dataset.palette = accent;
    root.dataset.reduceMotion = String(reducedMotion);
    root.style.setProperty('--green', theme.color);
    const palette = paletteFor(theme, dark);
    for (const name of ['side','main','card','active','border','ink']) {
      const property = `--palette-${name}`;
      if (palette) root.style.setProperty(property, palette[property]);
      else root.style.removeProperty(property);
    }
  }
  function applyScale() {
    const browserZoom = (window.devicePixelRatio || 1) / initialPixelRatio;
    document.documentElement.style.setProperty('--app-scale', String(scale / browserZoom));
  }
  media.addEventListener('change', () => { if (mode === 'system') apply(); });
  window.addEventListener('resize', applyScale);
  apply(); applyScale();
  return Object.freeze({
    get mode() { return mode; },
    get accent() { return accent; },
    get reducedMotion() { return reducedMotion; },
    get scale() { return scale; },
    get themes() { return themes; },
    colorFor(value) { return byId.get(value)?.color; },
    setMode(value) { if (!modes.includes(value)) throw new Error('Invalid appearance mode'); mode = value; apply(); },
    setAccent(value) { if (!byId.has(value)) throw new Error('Invalid appearance accent'); accent = value; apply(); },
    setReducedMotion(value) { if (typeof value !== 'boolean') throw new Error('Invalid motion preference'); reducedMotion = value; apply(); },
    setScale(value) { if (!Number.isFinite(value) || value < 0.75 || value > 1.5) throw new Error('Invalid interface scale'); scale = value; applyScale(); },
  });
}

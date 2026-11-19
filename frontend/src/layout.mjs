import {layout} from './layout-data.mjs';

// Build the static application DOM using element and text APIs, not HTML parsing.
export function mountLayout(document) {
  const root = document.getElementById('app-root');
  if (!root) throw new Error('ISA application root is missing');
  const svgNamespace = 'http://www.w3.org/2000/svg';
  function create(entry, inSvg = false) {
    if (typeof entry === 'string') return document.createTextNode(entry);
    const [tag, attributes, children] = entry;
    const svg = inSvg || tag === 'svg';
    const element = svg ? document.createElementNS(svgNamespace, tag) : document.createElement(tag);
    for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
    for (const child of children) element.appendChild(create(child, svg));
    return element;
  }
  const fragment = document.createDocumentFragment();
  for (const item of layout) fragment.appendChild(create(item));
  root.replaceWith(fragment);
}

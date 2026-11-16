import http from 'node:http';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const port = Number(process.env.PORT || 4174);
const types = {'.html': 'text/html; charset=utf-8', '.png': 'image/png', '.svg': 'image/svg+xml', '.js':'text/javascript; charset=utf-8', '.mjs':'text/javascript; charset=utf-8'};
function respond(response, status, type, body, head = false) {
  response.writeHead(status, {'Content-Type': type, 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff'});
  response.end(head ? undefined : body);
}
http.createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  if (request.method === 'POST' && url.pathname === '/api/changes') {
    const host=request.headers.host||'', origin=request.headers.origin||'', type=(request.headers['content-type']||'').split(';')[0].trim();
    if (!host || !['http://'+host,'https://'+host].includes(origin) || type !== 'application/json') return respond(response,403,'application/json','{"error":"Same-origin JSON request required"}');
    return respond(response,503,'application/json','{"error":"Git changes are not available in the development preview. Configure a workspace in the Rust server."}');
  }
  if (request.method === 'POST' && (url.pathname === '/api/decoy' || url.pathname === '/api/chat')) {
    const host = request.headers.host || '';
    const origin = request.headers.origin || '';
    const contentType = (request.headers['content-type'] || '').split(';')[0].trim();
    if (!host || !['http://' + host, 'https://' + host].includes(origin) || contentType !== 'application/json') return respond(response, 403, 'application/json', '{"error":"Same-origin JSON request required"}');
    let size = 0;
    const chunks = [];
    for await (const chunk of request) {
      size += chunk.length;
      if (size > 6 * 1024 * 1024) return respond(response, 413, 'application/json', '{"error":"Body too large"}');
      chunks.push(chunk);
    }
    try {
      const upstream = await fetch('http://127.0.0.1:4180/v1/chat', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({message:JSON.parse(Buffer.concat(chunks).toString('utf8')).message}), signal:AbortSignal.timeout(5000)});
      const body = await upstream.text();
      if (Buffer.byteLength(body) > 128 * 1024) throw new Error('Response too large');
      return respond(response, upstream.status, 'application/json; charset=utf-8', body);
    } catch { return respond(response, 502, 'application/json', '{"error":"Decoy provider unavailable"}'); }
  }
  if (request.method !== 'GET' && request.method !== 'HEAD') return respond(response, 405, 'text/plain', 'Method not allowed');
  const file = url.pathname === '/' ? '/index.html' : url.pathname;
  if (file !== '/index.html' && file !== '/assets/isa-logo.png' && file !== '/assets/isa-logo-transparent.png' && !['/assets/vendor/pdf.min.mjs','/assets/vendor/pdf.worker.min.mjs','/assets/vendor/mammoth.browser.js'].includes(file) && !/^\/assets\/providers\/[a-z-]+\.svg$/.test(file)) return respond(response, 404, 'text/plain', 'Not found');
  try {
    const body = await readFile(path.join(root, file));
    respond(response, 200, types[path.extname(file)] || 'application/octet-stream', body, request.method === 'HEAD');
  } catch { respond(response, 404, 'text/plain', 'Not found'); }
}).listen(port, '0.0.0.0', () => console.log(`ISA development preview listening on ${port}`));

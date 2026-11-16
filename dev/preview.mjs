import http from 'node:http';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const port = Number(process.env.PORT || 4174);
const types = {'.html': 'text/html; charset=utf-8', '.png': 'image/png', '.svg': 'image/svg+xml'};
function respond(response, status, type, body, head = false) {
  response.writeHead(status, {'Content-Type': type, 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff'});
  response.end(head ? undefined : body);
}
http.createServer(async (request, response) => {
  const url = new URL(request.url, 'http://localhost');
  if (request.method === 'POST' && (url.pathname === '/api/decoy' || url.pathname === '/api/chat')) {
    const host = request.headers.host || '';
    const origin = request.headers.origin || '';
    const contentType = (request.headers['content-type'] || '').split(';')[0].trim();
    if (!host || !['http://' + host, 'https://' + host].includes(origin) || contentType !== 'application/json') return respond(response, 403, 'application/json', '{"error":"Same-origin JSON request required"}');
    let size = 0;
    const chunks = [];
    for await (const chunk of request) {
      size += chunk.length;
      if (size > 128 * 1024) return respond(response, 413, 'application/json', '{"error":"Body too large"}');
      chunks.push(chunk);
    }
    try {
      const upstream = await fetch('http://127.0.0.1:4180/v1/chat', {method:'POST', headers:{'Content-Type':'application/json'}, body:Buffer.concat(chunks), signal:AbortSignal.timeout(5000)});
      const body = await upstream.text();
      if (Buffer.byteLength(body) > 128 * 1024) throw new Error('Response too large');
      return respond(response, upstream.status, 'application/json; charset=utf-8', body);
    } catch { return respond(response, 502, 'application/json', '{"error":"Decoy provider unavailable"}'); }
  }
  if (request.method !== 'GET' && request.method !== 'HEAD') return respond(response, 405, 'text/plain', 'Method not allowed');
  const file = url.pathname === '/' ? '/index.html' : url.pathname;
  if (file !== '/index.html' && file !== '/assets/isa-logo.png' && file !== '/assets/isa-logo-transparent.png' && !/^\/assets\/providers\/[a-z-]+\.svg$/.test(file)) return respond(response, 404, 'text/plain', 'Not found');
  try {
    const body = await readFile(path.join(root, file));
    respond(response, 200, types[path.extname(file)] || 'application/octet-stream', body, request.method === 'HEAD');
  } catch { respond(response, 404, 'text/plain', 'Not found'); }
}).listen(port, '0.0.0.0', () => console.log(`ISA development preview listening on ${port}`));

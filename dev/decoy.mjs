import http from 'node:http';

const limit = 128 * 1024;
http.createServer((request, response) => {
  const respond = (status, data) => {
    response.writeHead(status, {'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store'});
    response.end(JSON.stringify(data));
  };
  if (request.method !== 'POST' || request.url !== '/v1/chat') return respond(404, {error: 'Not found'});
  if (!request.headers['content-type']?.startsWith('application/json')) return respond(415, {error: 'Expected JSON'});
  let size = 0;
  const chunks = [];
  request.on('data', chunk => {
    size += chunk.length;
    if (size > limit) { request.destroy(); return; }
    chunks.push(chunk);
  });
  request.on('end', () => {
    try {
      const data = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      if (typeof data.message !== 'string' || !data.message.trim() || Buffer.byteLength(data.message) > 65536) return respond(400, {error: 'Invalid message'});
      respond(200, {provider: 'decoy', role: 'assistant', content: 'Message received by the decoy provider. No AI provider is configured, so this is an automatic acknowledgement rather than an AI response.'});
    } catch { respond(400, {error: 'Invalid JSON'}); }
  });
}).listen(4180, '127.0.0.1', () => console.log('Decoy provider listening on 127.0.0.1:4180'));

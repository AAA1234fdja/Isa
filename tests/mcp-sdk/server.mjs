// Interoperability fixture built on the official MCP TypeScript SDK.
// This server is deliberately local and only writes the explicitly supplied test marker.
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { writeFileSync } from 'node:fs';

const marker = process.argv[2];
if (!marker) throw new Error('missing test marker');
const server = new McpServer({ name: 'isa-interop-fixture', version: '1.0.0' });
server.registerTool('sdk_echo', {
  description: 'Echo a message and write a test marker.',
  inputSchema: { message: z.string().max(100) },
}, async ({ message }) => {
  writeFileSync(marker, JSON.stringify({ message }));
  return { content: [{ type: 'text', text: `SDK echo: ${message}` }] };
});
server.registerTool('sdk_error', {
  description: 'Return a tool-level error.',
  inputSchema: {},
}, async () => ({ content: [{ type: 'text', text: 'SDK tool failure' }], isError: true }));
await server.connect(new StdioServerTransport());

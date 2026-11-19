import assert from 'node:assert/strict';
import {normalizeEndpoint, readProviderUsage, summarizeUsage} from '../frontend/src/provider-utils.mjs';

assert.equal(normalizeEndpoint('https://example.com/v1/chat'), 'https://example.com/v1/chat');
for (const value of ['http://example.com', 'https://localhost/x', 'https://a.local/x', 'https://a.internal/x', 'https://127.0.0.1/x', 'https://[::1]/x', 'https://user:secret@example.com/x', 'https://example.com/x?key=secret', 'https://example.com/#fragment', 'not a URL']) assert.equal(normalizeEndpoint(value), null, value);
assert.deepEqual(readProviderUsage({prompt_tokens:10, completion_tokens:5, prompt_tokens_details:{cached_tokens:3}}), {input:10, output:5, total:15, cached:3});
assert.deepEqual(readProviderUsage({prompt_tokens:10, completion_tokens:5, total_tokens:20}), {input:10, output:5, total:20, cached:null});
for (const usage of [null, {}, {prompt_tokens:-1, completion_tokens:1}, {prompt_tokens:1.5, completion_tokens:2}, {prompt_tokens:Number.MAX_SAFE_INTEGER, completion_tokens:1}]) assert.equal(readProviderUsage(usage), null);
assert.deepEqual(summarizeUsage({usageEvents:[{usage:{input:10, output:5, total:15, cached:3}},{usage:{input:2,output:1,total:3}},{usage:null}]}), {input:12,output:6,total:18,cached:3,known:2,unknown:1,cachedUnknown:1});
assert.deepEqual(summarizeUsage(null), {input:0,output:0,total:0,cached:0,known:0,unknown:0,cachedUnknown:0});
console.log('Provider utility module validates endpoints and parses usage without UI state');

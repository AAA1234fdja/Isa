import {normalizeEndpoint, readProviderUsage, summarizeUsage} from '../src/provider-utils.mjs';

// Temporary bridge for legacy callers.
export const controller = Object.freeze({validProviderEndpoint: normalizeEndpoint, parseProviderUsage: readProviderUsage, tokenStats: summarizeUsage});

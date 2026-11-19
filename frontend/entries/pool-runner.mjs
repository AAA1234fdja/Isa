import {createPoolRunner, normalizeWorkflow} from '../src/pool-runner.mjs';
// Temporary bridge for the classic UI and supervision handlers.
export const controller = Object.freeze({createPoolRunner, normalizeWorkflow});

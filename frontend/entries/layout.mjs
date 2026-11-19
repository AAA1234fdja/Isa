import {mountLayout} from '../src/layout.mjs';
// Mount synchronously before the remaining application scripts initialize.
mountLayout(document);
export const controller = Object.freeze({mountLayout});

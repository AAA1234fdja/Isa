import {createAppearance} from '../src/appearance.mjs';

// Temporary bridge for classic scripts while the rest of the UI is migrated.
export const controller = createAppearance({window, document});

import {createSidebar} from '../src/sidebar.mjs';

// Temporary bridge for classic scripts while the rest of the UI is migrated.
export const controller = createSidebar({window, document});

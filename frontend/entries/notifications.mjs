import {createNotifications} from '../src/notifications.mjs';

// Temporary bridge for classic scripts while the rest of the UI is migrated.
export const controller = createNotifications({window, document});

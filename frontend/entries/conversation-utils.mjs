import {formatLocalDay, cleanAgentSteps, cleanRunActivity, parseChanges, classifyAttachment, buildContextHistory} from '../src/conversation-utils.mjs';

// Temporary bridge for legacy callers.
export const controller = Object.freeze({localDay: formatLocalDay, normalizeAgentSteps: cleanAgentSteps, normalizeRunActivity: cleanRunActivity, splitChanges: parseChanges, attachmentKind: classifyAttachment, contextHistory: buildContextHistory});

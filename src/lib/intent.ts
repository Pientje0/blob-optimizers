/**
 * One-shot navigation intents: a page sets one before navigating and the target page consumes it,
 * e.g. the Dashboard's quick action opens the default gaming preset's preview on the Gaming page.
 */
export type Intent = { kind: "preview-preset"; preset: "balanced" | "gaming" | "maximum" };

let pending: Intent | null = null;

export function setIntent(intent: Intent) {
  pending = intent;
}

export function takeIntent<K extends Intent["kind"]>(kind: K): Extract<Intent, { kind: K }> | null {
  if (pending?.kind !== kind) return null;
  const i = pending as Extract<Intent, { kind: K }>;
  pending = null;
  return i;
}

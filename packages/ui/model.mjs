export const THEMES = ['white-acrylic', 'black-acrylic', 'liquid-glass'];
export const LANGUAGES = ['en', 'zh-CN'];
export const CLOSE = ['ask', 'tray', 'exit'];
export const PAGES = ['home', 'settings', 'market', 'onboarding'];

export function fallback(messages, language, key) {
  const selected = messages[language] ?? messages.en;
  const value = selected?.[key];
  if (typeof value === 'string' && value.length > 0) return value;
  const english = messages.en?.[key];
  return typeof english === 'string' && english.length > 0 ? english : key;
}

export function motionEnabled(visibility, reducedMotion) {
  return visibility === 'visible' && reducedMotion !== true;
}

export function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  })[character]);
}

export function sameKeys(left, right) {
  const a = Object.keys(left).sort();
  const b = Object.keys(right).sort();
  return a.length === b.length && a.every((key, index) => key === b[index] && left[key] && right[key]);
}

import i18n from '../../i18n';

const MODIFIERS = {
  Control: 'common.keys.control',
  Shift: 'common.keys.shift',
  Alt: 'common.keys.alt',
  Meta: 'common.keys.meta',
} as const;
const isModifier = (key: string): key is keyof typeof MODIFIERS => key in MODIFIERS;

/** "Control+Shift+8" → ["Ctrl", "Maj", "8"] in French, ["Ctrl", "Shift", "8"] in English (Windows names). */
export function shortcutKeys(shortcut: string): string[] {
  return shortcut.split('+').map(key => (isModifier(key) ? i18n.t(MODIFIERS[key]) : key.toUpperCase()));
}

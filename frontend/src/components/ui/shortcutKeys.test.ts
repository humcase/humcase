import { describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { shortcutKeys } from './shortcutKeys';

describe('shortcutKeys', () => {
  it('names the keys in the interface language', async () => {
    expect(shortcutKeys('Control+Shift+8')).toEqual(['Ctrl', 'Maj', '8']);
    await i18n.changeLanguage('en');
    try {
      expect(shortcutKeys('Control+Shift+8')).toEqual(['Ctrl', 'Shift', '8']);
    } finally {
      await i18n.changeLanguage('fr');
    }
  });
});

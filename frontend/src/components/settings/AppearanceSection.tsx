import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowCounterClockwise as RotateCcw } from '@phosphor-icons/react';
import { MAX_FONT_SCALE, MIN_FONT_SCALE, getEditorFontScale, setEditorFontScale } from '../../utils/prefs';
import { LanguagePreference } from '../LanguagePreference';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

/** Appearance: language, theme and the size of the text being written. */
export function AppearanceSection({ theme, onThemeChange }: { theme: 'dark' | 'light'; onThemeChange: (theme: 'dark' | 'light') => void }) {
  const { t } = useTranslation();
  const [fontScale, setFontScale] = useState<number>(() => getEditorFontScale());
  const changeFontScale = (percent: number) => setFontScale(setEditorFontScale(percent / 100));
  return (
    <div className="settings-panel">
      <div className="settings-section-heading"><h3>{t('settings.appearance.heading')}</h3></div>
      <LanguagePreference />
      <Card padding="none" className="settings-card settings-preference-row" role="group" aria-labelledby="settings-theme-label">
        <div className="settings-preference-copy"><h4 id="settings-theme-label">{t('settings.appearance.themeTitle')}</h4></div>
        <div className="settings-theme-options">
          <label><input type="radio" name="settings-theme" value="dark" checked={theme === 'dark'} onChange={() => onThemeChange('dark')} /><span>{t('settings.appearance.dark')}</span></label>
          <label><input type="radio" name="settings-theme" value="light" checked={theme === 'light'} onChange={() => onThemeChange('light')} /><span>{t('settings.appearance.light')}</span></label>
        </div>
      </Card>
      <Card padding="none" className="settings-card settings-preference-row">
        <label htmlFor="editor-font-scale" className="settings-label block mb-3">
          {t('settings.appearance.fontSize')}
        </label>
        <div className="flex items-center gap-3">
          <span className="text-xs text-muted-foreground w-12 text-right font-mono">{Math.round(fontScale * 100)} %</span>
          <input
            id="editor-font-scale"
            type="range"
            min={MIN_FONT_SCALE * 100}
            max={MAX_FONT_SCALE * 100}
            step={5}
            value={Math.round(fontScale * 100)}
            onChange={(e) => changeFontScale(Number(e.target.value))}
            className="flex-1 accent-primary cursor-pointer"
          />
          <Button variant="secondary"
            onClick={() => changeFontScale(100)}
            title={t('settings.appearance.resetTitle')}
            aria-label={t('settings.appearance.resetAria')}
          >
            <RotateCcw className="w-4 h-4" />
          </Button>
        </div>
        <div className="settings-reading-preview" style={{ fontSize: `calc(var(--workspace-text-body) * ${fontScale})` }}><strong>{t('settings.appearance.previewTitle')}</strong><p>{t('settings.appearance.previewText')}</p></div>
      </Card>
    </div>
  );
}

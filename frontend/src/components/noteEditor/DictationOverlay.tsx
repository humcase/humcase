import { useTranslation } from 'react-i18next';
import { Card } from '../ui/Card';

interface DictationOverlayProps {
  liveTranscript: string;
  interimTranscript: string;
}

/** Live dictation banner: confirmed text then provisional text. */
export function DictationOverlay({ liveTranscript, interimTranscript }: DictationOverlayProps) {
  const { t } = useTranslation();
  return (
          <Card active className="note-editor-dictation">
            <div className="note-editor-dictation__heading">
              <span className="note-editor-dictation__dot" aria-hidden="true"></span>
              {t('editor.dictation.live')}
            </div>
            <div className="note-editor-dictation__text">
              {liveTranscript}
              <span className="note-editor-dictation__interim">{interimTranscript}</span>
            </div>
          </Card>
  );
}

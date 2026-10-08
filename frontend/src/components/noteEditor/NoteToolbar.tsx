import { useTranslation } from 'react-i18next';
import { PencilSimple as Edit2, Image as ImageIcon, Microphone as Mic, Square, CircleNotch as Loader2, Columns, TreeStructure, Plant, ChatsCircle } from '@phosphor-icons/react';
import { NoteTemplateMenu } from '../NoteTemplateMenu';
import { requestOpenSettings } from '../../hooks/useAiReady';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { SegmentedControl } from '../ui/SegmentedControl';

export type EditorView = 'edit' | 'diff';

/**
 * The commands above the text: the switch to compare with the raw dictation, Structure, Faire
 * germer, the templates (Work space) and dictation. `flex-wrap`: in mini mode (400 px) or a narrow window,
 * the commands wrap to the next line instead of overflowing; the paste hint disappears when space
 * runs out.
 */
export function NoteToolbar({
  hasRaw, viewMode, onViewModeChange, aiReady, canStructure, onStructure, canGerminate, germinating, onGerminate, onReflect, onTemplate, isRecording, isProcessingVoice, onStartDictation, onStopDictation,
}: {
  /** The case keeps a raw dictation: it can be compared with the text. */
  hasRaw: boolean;
  viewMode: EditorView;
  onViewModeChange: (view: EditorView) => void;
  /** AI connected in Settings? `false` locks Structure (visible, explained); `null` = unknown. */
  aiReady: boolean | null;
  canStructure: boolean;
  onStructure: () => void;
  /** Faire germer: the panel of proposed sub-cases is open (the button shows it pressed). */
  canGerminate: boolean;
  germinating: boolean;
  onGerminate: () => void;
  /** Think it through with Humcase, starting from this case (the panel opens on the right). */
  onReflect?: () => void;
  /** Templates are offered in the Work space only. */
  onTemplate?: (type: string) => void;
  isRecording: boolean;
  isProcessingVoice: boolean;
  onStartDictation: () => void;
  onStopDictation: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="note-editor-toolbar flex flex-wrap items-center px-6 pt-4 gap-x-2 gap-y-2">
      {hasRaw && <SegmentedControl size="sm" label={t('editor.view.label')} value={viewMode} onValueChange={onViewModeChange} items={[
        { value: 'edit', label: t('editor.view.rich'), icon: <Edit2 aria-hidden="true" /> },
        { value: 'diff', label: t('editor.view.compare'), icon: <Columns aria-hidden="true" /> },
      ]} />}
      <div className="ml-auto flex flex-wrap items-center justify-end gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={onStructure}
          disabled={!canStructure || aiReady === false}
          title={aiReady === false ? t('editor.structure.needsAi') : t('editor.structure.buttonTitle')}
        >
          <TreeStructure className="text-primary" aria-hidden="true" /> {t('editor.structure.button')}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={onGerminate}
          disabled={!canGerminate || aiReady === false}
          aria-pressed={germinating}
          title={aiReady === false ? t('editor.germinate.needsAi') : t('editor.germinate.buttonTitle')}
        >
          <Plant className="text-primary" aria-hidden="true" /> {t('editor.germinate.button')}
        </Button>
        {onReflect && (
          <Button size="sm" variant="outline" onClick={onReflect} disabled={aiReady === false} title={t('editor.reflect.buttonTitle')}>
            <ChatsCircle className="text-primary" aria-hidden="true" /> {t('editor.reflect.button')}
          </Button>
        )}
        {aiReady === false && (
          <Button size="sm" variant="ghost" onClick={requestOpenSettings} title={t('editor.structure.needsAi')}>
            {t('editor.structure.connectAi')}
          </Button>
        )}
        {onTemplate && <NoteTemplateMenu onSelect={onTemplate} />}
        {isProcessingVoice ? (
          <Badge tone="accent" role="status" icon={<Loader2 size={13} className="animate-spin" aria-hidden="true" />}>{t('editor.dictation.processing')}</Badge>
        ) : isRecording ? (
          <Button size="sm" onClick={onStopDictation} variant="destructive">
            <Square aria-hidden="true" /> {t('editor.dictation.stop')}
          </Button>
        ) : (
          <Button size="sm" variant="outline" onClick={onStartDictation}>
            <Mic className="text-primary" aria-hidden="true" /> {t('editor.dictation.start')}
          </Button>
        )}
        <span className="text-xs text-muted-foreground hidden min-[900px]:flex items-center gap-1 ml-2">
          <ImageIcon className="w-3 h-3" /> {t('editor.dictation.pasteHint')}
        </span>
      </div>
    </div>
  );
}

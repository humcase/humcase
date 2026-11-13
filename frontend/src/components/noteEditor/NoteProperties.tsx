import { useTranslation } from 'react-i18next';
import { GitBranch, X } from '@phosphor-icons/react';
import { NOTE_TYPES, type Note, type NoteType } from '../../types';
import { TYPE_LABELS } from '../../utils/notePresentation';
import { DueDateField } from '../DueDateField';
import { NoteStatusControl } from '../NoteFollowUp';
import TagInput from '../TagInput';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';

interface NotePropertiesProps {
  note: Note;
  onNoteUpdated: () => void;
  priority: string;
  onPriorityChange: (priority: string) => void;
  dueAt: string;
  onDueChange: (due: string) => void;
  onClearDue: () => void;
  noteType: NoteType;
  onTypeChange: (type: NoteType) => void;
  suggestedType: NoteType | null;
  onAcceptSuggestion: (type: NoteType) => void;
  onDismissSuggestion: (type: NoteType) => void;
  tags: string[];
  onTagsChange: (tags: string[]) => void;
  availableTags: string[];
}

/** Organization properties: status, priority, due date, type, Git branch and labels. */
export function NoteProperties({
  note,
  onNoteUpdated,
  priority,
  onPriorityChange,
  dueAt,
  onDueChange,
  onClearDue,
  noteType,
  onTypeChange,
  suggestedType,
  onAcceptSuggestion,
  onDismissSuggestion,
  tags,
  onTagsChange,
  availableTags,
}: NotePropertiesProps) {
  const { t } = useTranslation();
  return (
    <>
          <h2 className="note-editor-section-label">{t('editor.properties.organization')}</h2>
          <div className="note-editor-property note-editor-status"><span>{t('editor.properties.status')}</span><NoteStatusControl key={note.id} note={note} onUpdated={onNoteUpdated} /></div>
          <div className="note-editor-metadata flex items-center gap-3 mt-2 text-sm text-muted-foreground flex-wrap">
            <label className="note-editor-property"><span>{t('editor.properties.priority')}</span>
            <select
              aria-label={t('editor.properties.priority')}
              value={priority}
              onChange={(e) => onPriorityChange(e.target.value)}
              className="ui-field"
            >
              <option value="">{t('editor.properties.noPriority')}</option>
              <option value="P0">{t('editor.properties.p0')}</option>
              <option value="P1">{t('editor.properties.p1')}</option>
              <option value="P2">{t('editor.properties.p2')}</option>
              <option value="P3">{t('editor.properties.p3')}</option>
            </select>
            </label>
            <div className="note-editor-due flex items-center gap-1" title={t('editor.properties.dueTitle')}>
              <span>{t('editor.properties.due')}</span>
              <DueDateField key={note.id} value={dueAt} onChange={onDueChange} />
              {dueAt && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={onClearDue}
                  title={t('editor.properties.clearDue')}
                  aria-label={t('editor.properties.clearDue')}
                  className="note-editor-clear"
                >
                  <X aria-hidden="true" />
                </Button>
              )}
            </div>
            <label className="note-editor-property"><span>{t('editor.properties.type')}</span>
            <select
              aria-label={t('editor.properties.typeLabel')}
              value={noteType}
              onChange={(e) => onTypeChange(e.target.value as NoteType)}
              className="ui-field"
            >
              {NOTE_TYPES.map(type => <option key={type} value={type}>{TYPE_LABELS[type]}</option>)}
            </select>
            </label>
            {suggestedType && String(suggestedType) !== String(noteType) && (
              <div className="flex items-center space-x-1 animate-fade-in">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => onAcceptSuggestion(suggestedType)}
                >
                  {t('editor.properties.suggest', { type: TYPE_LABELS[suggestedType] })}
                </Button>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onDismissSuggestion(suggestedType)}
                  aria-label={t('editor.properties.dismissSuggestion')}
                  className="note-editor-clear"
                >
                  <X aria-hidden="true" />
                </Button>
              </div>
            )}
            {note.git_branch && (
              <Badge variant="outline" className="note-editor-branch" title={note.git_branch} icon={<GitBranch size={15} aria-hidden="true" />}>
                {note.git_branch}
              </Badge>
            )}

            <div className="note-editor-tags">
              <TagInput tags={tags} onChange={onTagsChange} availableTags={availableTags} />
            </div>
          </div>
    </>
  );
}

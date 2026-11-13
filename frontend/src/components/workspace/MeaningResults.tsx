import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { MeaningHit, MeaningStatus, Note } from '../../types';
import { List, ListRow } from '../ui/List';
import { Count, TypeBadge } from '../ui/Badge';
import { meaningRows } from '../../utils/meaningRows';
import './meaning-results.css';

/**
 * "Close in meaning", under the word search: the cases of the page closest in meaning to the
 * search that the words did not find. When the words found cases, only the cases that stand out
 * clearly are added (an unrelated search then adds nothing); when they found none, the closest
 * ones help. While the cases are still being read, a discreet line says how far it is; when
 * search by meaning is off or unavailable, nothing is shown.
 */
export function MeaningResults({ hits, status, cases, wordIds, onOpen, locate }: {
  hits: MeaningHit[];
  status: MeaningStatus | null;
  /** Cases the page shows (space, project, archives): hits outside are left out. */
  cases: ReadonlyMap<string, Note>;
  /** Cases the word search already found. */
  wordIds: ReadonlySet<string> | null;
  onOpen: (note: Note) => void;
  locate?: (note: Note) => ReactNode;
}) {
  const { t } = useTranslation();
  const shown = meaningRows(hits, cases, wordIds);
  const reading = status?.state === 'reading' && status.total > 0;
  if (shown.length === 0 && !reading) return null;
  return (
    <section className="workspace-meaning" aria-labelledby="workspace-meaning-heading">
      <div className="workspace-meaning__head">
        <h2 id="workspace-meaning-heading" className="workspace-meaning__title">{t('workspace.meaning.heading')}</h2>
        {shown.length > 0 && <Count value={shown.length} />}
        {reading && <span className="workspace-meaning__reading" role="status">{t('workspace.meaning.reading', { done: status.done, total: status.total })}</span>}
      </div>
      {shown.length > 0 && (
        <List variant="boxed" label={t('workspace.meaning.heading')}>
          {shown.map(({ hit, note }) => (
            <ListRow key={note.id} size="sm" title={note.title || t('common.untitled')}
              leading={<TypeBadge type={note.note_type} />}
              meta={locate?.(note)}
              trailing={<span className="workspace-meaning__closeness">{hit.very_close ? t('workspace.meaning.veryClose') : t('workspace.meaning.close')}</span>}
              onClick={() => onOpen(note)} actionLabel={note.title} />
          ))}
        </List>
      )}
    </section>
  );
}

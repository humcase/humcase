import { useTranslation } from 'react-i18next';
import { ArrowCounterClockwise, CaretDown, CheckCircle, LinkSimple, Plant, Quotes, Robot, Warning, X } from '@phosphor-icons/react';
import type { NoteType, Proposal } from '../../types';
import { TYPE_ICONS, TYPE_LABELS } from '../../utils/notePresentation';
import { Button } from '../ui/Button';
import { Menu, MenuItem } from '../ui/Menu';
import type { Germinating, ProposalDraft } from './useGermination';
import './germinate.css';

/** Natures a proposed step can take, in the menu's order. */
const STEP_NATURES: NoteType[] = ['todo', 'question', 'research', 'decision', 'idea'];

/**
 * Faire germer, inside a case: the next steps Humcase (and agents) propose as sub-cases, each
 * with the passage it comes from. The person checks, corrects, refuses; nothing is created before
 * they answer. A doubt of Jev and a case that may already cover a step are shown beside it.
 */
export function GerminatePanel({ germination }: { germination: Germinating }) {
  const { t } = useTranslation();
  const { stage, proposals, drafts, error } = germination;

  if (stage === 'reading') {
    return (
      <section className="germinate" data-state="reading" aria-live="polite">
        <p className="germinate__title"><Plant size={18} aria-hidden="true" />{t('editor.germinate.reading')}</p>
        <div className="germinate__reading" aria-hidden="true"><span /></div>
        <p className="germinate__fine">{t('editor.germinate.readingHint')}</p>
      </section>
    );
  }

  if (stage === 'closed') {
    return (
      <section className="germinate" data-state="closed">
        <Plant size={18} className="germinate__icon" aria-hidden="true" />
        <div className="germinate__band-text">
          <p className="germinate__title">{t('editor.germinate.pending', { count: proposals.length })}</p>
          <p className="germinate__hint">{t('editor.germinate.pendingHint')}</p>
        </div>
        <Button size="sm" variant="outline" onClick={germination.reopen}>{t('editor.germinate.resume')}</Button>
      </section>
    );
  }

  if (stage === 'done' && germination.outcome) {
    const { created, placed, skipped, refused } = germination.outcome;
    const parts = [
      created.length > 0 && t('editor.germinate.created', { count: created.length }),
      placed.length > 0 && t('editor.germinate.placed', { count: placed.length }),
      skipped > 0 && t('editor.germinate.skipped', { count: skipped }),
      refused > 0 && t('editor.germinate.refused', { count: refused }),
    ].filter(Boolean);
    return (
      <section className="germinate" data-state="done" aria-live="polite">
        <CheckCircle size={20} className="germinate__icon" aria-hidden="true" />
        <div className="germinate__band-text">
          <p className="germinate__title">{parts.length > 0 ? parts.join(' · ') : t('editor.germinate.nothingDone')}</p>
          <p className="germinate__hint">{t('editor.germinate.doneHint')}</p>
          {error && <p role="alert" className="germinate__error">{error}</p>}
        </div>
        <Button size="sm" variant="outline" onClick={() => { void germination.undo(); }} disabled={germination.busy}>
          <ArrowCounterClockwise aria-hidden="true" />{t('editor.germinate.undo')}
        </Button>
      </section>
    );
  }

  if (stage !== 'open') {
    return error ? <p role="alert" className="germinate__error germinate__error--alone">{t('editor.germinate.error', { error })}</p> : null;
  }

  const answers = germination.answers;
  const created = answers.filter(a => a.choice === 'create').length;
  const placed = answers.filter(a => a.choice === 'place').length;
  const label = created + placed === 0
    ? (answers.length > 0 ? t('editor.germinate.recordRefusals') : t('editor.germinate.close'))
    : [created > 0 && t('editor.germinate.create', { count: created }), placed > 0 && t('editor.germinate.place', { count: placed })].filter(Boolean).join(' · ');

  return (
    <section className="germinate" data-state="open" aria-labelledby="germinate-title">
      <header className="germinate__head">
        <Plant size={20} className="germinate__icon" aria-hidden="true" />
        <div className="germinate__head-text">
          <h2 id="germinate-title" className="germinate__heading">
            {t('editor.germinate.title')}
            <span className="germinate__count">{t('editor.germinate.count', { count: proposals.length })}</span>
          </h2>
          <p className="germinate__hint">{proposals.length > 0 ? t('editor.germinate.hint') : t('editor.germinate.empty')}</p>
        </div>
        <Button size="icon-sm" variant="ghost" onClick={germination.close} aria-label={t('editor.germinate.closeLabel')} title={t('editor.germinate.closeLabel')}>
          <X aria-hidden="true" />
        </Button>
      </header>

      {proposals.length > 0 && (
        <ul className="germinate__list">
          {proposals.map(proposal => drafts[proposal.id] && (
            <ProposalRow key={proposal.id} proposal={proposal} draft={drafts[proposal.id]} germination={germination} />
          ))}
        </ul>
      )}

      {error && <p role="alert" className="germinate__error">{error}</p>}

      <footer className="germinate__foot">
        <p className="germinate__fine">
          {t('editor.germinate.verified')}
          {germination.checked && <><br />{t('editor.germinate.checkedByJev')}</>}
          {germination.dropped > 0 && <><br />{t('editor.germinate.dropped', { count: germination.dropped })}</>}
        </p>
        {proposals.length > 0 && <Button size="sm" variant="ghost" onClick={germination.refuseAll} disabled={germination.busy}>{t('editor.germinate.refuseAll')}</Button>}
        <Button size="sm" onClick={() => { if (answers.length > 0) void germination.submit(); else germination.close(); }} disabled={germination.busy}>{label}</Button>
      </footer>
    </section>
  );
}

function ProposalRow({ proposal, draft, germination }: { proposal: Proposal; draft: ProposalDraft; germination: Germinating }) {
  const { t } = useTranslation();
  const Icon = TYPE_ICONS[draft.nature] ?? TYPE_ICONS.todo;
  const edited = draft.nature !== proposal.nature || draft.title.trim() !== proposal.title;
  const near = proposal.near;
  const lit = germination.litPassage === proposal.passage;
  const nearText = !near ? '' : near.place === 'here'
    ? t('editor.germinate.near.here')
    : near.place === 'movable'
      ? t('editor.germinate.near.movable')
      : near.parent_title ? t('editor.germinate.near.elsewhere', { parent: near.parent_title }) : t('editor.germinate.near.otherProject');
  const choices: ProposalDraft['nearChoice'][] = near?.place === 'movable' ? ['place', 'create'] : ['skip', 'create'];
  // A plain button, drawn as the nature chip: the menu opens our own list, never the system's.
  const natureButton = (
    <button type="button" className="germinate__nature" data-type={draft.nature} aria-label={t('editor.germinate.natureOf', { nature: TYPE_LABELS[draft.nature] })}>
      <Icon size={16} aria-hidden="true" />{TYPE_LABELS[draft.nature]}<CaretDown size={12} aria-hidden="true" />
    </button>
  );
  return (
    <li
      className="germinate__row"
      data-kept={draft.keep || undefined}
      data-lit={lit || undefined}
      onMouseEnter={() => germination.hover(proposal.passage)}
      onMouseLeave={() => germination.hover(null)}
    >
      <div className="germinate__line">
        <input
          type="checkbox"
          className="germinate__check"
          checked={draft.keep}
          onChange={event => germination.update(proposal.id, { keep: event.target.checked })}
          aria-label={t('editor.germinate.keep', { title: draft.title })}
        />
        <Menu label={t('editor.germinate.natureLabel')} trigger={natureButton}>
          {STEP_NATURES.map(nature => {
            const NatureIcon = TYPE_ICONS[nature];
            return (
              <MenuItem key={nature} onSelect={() => germination.update(proposal.id, { nature })} icon={<span className="germinate__menu-icon" data-type={nature}><NatureIcon size={16} aria-hidden="true" /></span>}>
                {TYPE_LABELS[nature]}
              </MenuItem>
            );
          })}
        </Menu>
        {/* A title wraps on narrow windows: a one-line field grown to its text. */}
        <textarea
          rows={1}
          className="germinate__title-input"
          value={draft.title}
          onChange={event => germination.update(proposal.id, { title: event.target.value.replace(/\n/g, ' ') })}
          onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); event.currentTarget.blur(); } }}
          aria-label={t('editor.germinate.titleLabel')}
        />
        {edited && <span className="germinate__tag">{t('editor.germinate.edited')}</span>}
        {proposal.proposed_by && <span className="germinate__tag"><Robot size={13} aria-hidden="true" />{t('editor.germinate.proposedBy', { agent: proposal.proposed_by })}</span>}
      </div>
      <button type="button" className="germinate__quote" onClick={() => germination.showPassage(proposal.passage)} title={proposal.reason ? `${t('editor.germinate.showPassage')} · ${proposal.reason}` : t('editor.germinate.showPassage')}>
        <Quotes size={15} aria-hidden="true" /><span>{t('editor.germinate.quoted', { passage: proposal.passage })}</span>
      </button>
      {near && draft.keep && (
        <div className="germinate__near">
          <p className="germinate__near-text"><LinkSimple size={16} aria-hidden="true" /><span>{nearText} <b>{near.title}</b></span></p>
          <div className="germinate__choice" role="group" aria-label={t('editor.germinate.nearChoice.label')}>
            {choices.map(choice => (
              <button key={choice} type="button" aria-pressed={draft.nearChoice === choice} onClick={() => germination.update(proposal.id, { nearChoice: choice })}>
                {t(`editor.germinate.nearChoice.${choice}`)}
              </button>
            ))}
          </div>
        </div>
      )}
      {proposal.doubt && (
        <p className="germinate__doubt"><Warning size={16} aria-hidden="true" /><span><b>{t('editor.germinate.doubt')}</b> · {t('editor.germinate.doubtText')}</span></p>
      )}
    </li>
  );
}

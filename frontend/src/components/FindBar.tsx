import { useEffect, useReducer, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Editor } from '@tiptap/react';
import { CaretDown, CaretUp, X } from '@phosphor-icons/react';
import { searchKey, searchState } from './FindInNote';
import { SearchField } from './ui/SearchField';
import { Button } from './ui/Button';
import './find-bar.css';

/**
 * "Find in note" bar: Ctrl+F on the note page opens it (the selected text
 * becomes the search), Enter / Shift+Enter move from one occurrence to the next, Escape closes it and
 * selects the current occurrence in the text. Nothing is written to the note.
 */
export function FindBar({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState('');
  const field = useRef<HTMLInputElement>(null);
  // Decorations live in the ProseMirror state: the counter is reread on every transaction.
  const [, refresh] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    editor.on('transaction', refresh);
    return () => { editor.off('transaction', refresh); };
  }, [editor]);

  const submit = (meta: { query?: string; currentValue?: number }) => {
    if (editor.isDestroyed) return;
    editor.view.dispatch(editor.state.tr.setMeta(searchKey, meta));
    // The current occurrence stays visible, without moving the cursor or the field's focus.
    requestAnimationFrame(() => { if (!editor.isDestroyed) editor.view.dom.querySelector('.find-match--current')?.scrollIntoView?.({ block: 'center' }); });
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'f' || event.shiftKey || event.altKey) return;
      // Only when this note is displayed and no modal window covers it.
      const dom = editor.isDestroyed ? null : editor.view.dom;
      if (!dom || !dom.isConnected || dom.getClientRects().length === 0) return;
      if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      event.preventDefault();
      const { from, to } = editor.state.selection;
      const selection = from !== to ? editor.state.doc.textBetween(from, to, ' ') : '';
      const initialQuery = selection && selection.length <= 80 && !selection.includes('\n') ? selection : query;
      setIsOpen(true);
      setQuery(initialQuery);
      submit({ query: initialQuery, currentValue: 0 });
      requestAnimationFrame(() => { field.current?.focus(); field.current?.select(); });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const closeBar = (keepSelection: boolean) => {
    const state = searchState(editor.state);
    const occurrence = state?.occurrences[state.currentValue];
    setIsOpen(false);
    submit({ query: '', currentValue: 0 });
    if (keepSelection && occurrence) editor.chain().focus().setTextSelection(occurrence).run();
    else editor.commands.focus();
  };

  if (!isOpen) return null;
  const state = searchState(editor.state);
  const total = state?.occurrences.length ?? 0;
  const currentValue = state?.currentValue ?? 0;
  const goTo = (step: number) => { if (total > 0) submit({ currentValue: (currentValue + step + total) % total }); };

  return (
    <div className="find-bar" role="search" aria-label={t('editor.find.label')}>
      <SearchField ref={field} id="find-in-note" label={t('editor.find.field')} className="find-bar__field" placeholder={t('editor.find.field')}
        value={query} aria-invalid={query.trim() !== '' && total === 0 ? true : undefined}
        onChange={event => { setQuery(event.target.value); submit({ query: event.target.value, currentValue: 0 }); }}
        onKeyDown={event => {
          if (event.key === 'Enter') { event.preventDefault(); goTo(event.shiftKey ? -1 : 1); }
          else if (event.key === 'Escape') { event.preventDefault(); closeBar(true); }
        }} />
      <span className="find-bar__count" aria-live="polite">{query.trim() === '' ? '' : total === 0 ? t('editor.find.noResults') : `${currentValue + 1} / ${total}`}</span>
      <Button type="button" variant="ghost" size="icon-sm" aria-label={t('editor.find.previous')} title={t('editor.find.previousTitle')} disabled={total === 0} onClick={() => goTo(-1)}><CaretUp aria-hidden="true" /></Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label={t('editor.find.next')} title={t('editor.find.nextTitle')} disabled={total === 0} onClick={() => goTo(1)}><CaretDown aria-hidden="true" /></Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label={t('editor.find.close')} title={t('editor.find.closeTitle')} onClick={() => closeBar(false)}><X aria-hidden="true" /></Button>
    </div>
  );
}

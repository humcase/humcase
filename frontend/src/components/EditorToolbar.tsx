import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { type Editor, useEditorState } from '@tiptap/react';
import { Popover, Toolbar } from 'radix-ui';
import { TextB, TextItalic, TextStrikethrough, ListBullets, ListNumbers, ListChecks, Quotes, Code, LinkSimple, ArrowCounterClockwise, ArrowClockwise, Check, X } from '@phosphor-icons/react';
import { Button } from './ui/Button';
import { Hint, HintGroup } from './ui/Hint';
import { EDITOR_SHORTCUTS } from './editorShortcuts';
import './editor-toolbar.css';

/** Commands on the existing instance: no conversion or parallel save. */
export function EditorToolbar({ editor }: { editor: Editor }) {
  const { t } = useTranslation();
  const mounted = useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [linkOpen, setLinkOpen] = useState(false);
  const [url, setUrl] = useState('');
  const [error, setError] = useState('');
  const state = useEditorState({ editor, selector: ({ editor: e }) => ({
    bold:e.isActive('bold'), italic:e.isActive('italic'), strike:e.isActive('strike'),
    bullet:e.isActive('bulletList'), ordered:e.isActive('orderedList'), task:e.isActive('taskList'),
    quote:e.isActive('blockquote'), code:e.isActive('codeBlock'), link:e.isActive('link'),
    heading: [1,2,3].find(level=>e.isActive('heading',{level})) || 0,
    undo:e.can().undo(), redo:e.can().redo(),
  }) });
  // `shortcut`: actual editor shortcut (Tiptap), verified by EditorToolbar.test.tsx.
  const action = (label:string, shortcut:string|undefined, Icon:typeof TextB, run:()=>void, active=false, disabled=false) => (
    <Hint key={label} label={label} shortcut={shortcut}>
      <Toolbar.Button type="button" className="editor-tool" aria-label={label} aria-keyshortcuts={shortcut} aria-pressed={active} disabled={disabled}
        onMouseDown={event=>event.preventDefault()} onClick={run}><Icon size={18}/></Toolbar.Button>
    </Hint>
  );
  const applyLink = () => {
    const href=url.trim();
    if (!href) { editor.chain().focus().extendMarkRange('link').unsetLink().run(); setLinkOpen(false); return; }
    if (!/^(https?:\/\/|mailto:)/i.test(href)) { setError(t('editor.toolbar.link.invalid')); return; }
    editor.chain().focus().extendMarkRange('link').setLink({href}).run();setLinkOpen(false);
  };
  return <HintGroup>
    <Toolbar.Root className="editor-format-toolbar" aria-label={t('editor.toolbar.label')}>
      <div className="editor-tool-group">
        {action(t('editor.toolbar.undo'),EDITOR_SHORTCUTS.undo,ArrowCounterClockwise,()=>editor.chain().focus().undo().run(),false,!state.undo)}
        {action(t('editor.toolbar.redo'),EDITOR_SHORTCUTS.redo,ArrowClockwise,()=>editor.chain().focus().redo().run(),false,!state.redo)}
      </div>
      <Toolbar.Separator className="editor-tool-separator"/>
      <select className="ui-field ui-field--sm editor-block-select" aria-label={t('editor.toolbar.paragraphStyle')} value={state.heading}
        onChange={event=>{ const level=Number(event.target.value); if(level)editor.chain().focus().setHeading({level:level as 1|2|3}).run();else editor.chain().focus().setParagraph().run(); }}>
        <option value="0">{t('editor.toolbar.text')}</option><option value="1">{t('editor.toolbar.heading1')}</option><option value="2">{t('editor.toolbar.heading2')}</option><option value="3">{t('editor.toolbar.heading3')}</option>
      </select>
      <Toolbar.Separator className="editor-tool-separator"/>
      <div className="editor-tool-group">
        {action(t('editor.toolbar.bold'),EDITOR_SHORTCUTS.bold,TextB,()=>editor.chain().focus().toggleBold().run(),state.bold)}
        {action(t('editor.toolbar.italic'),EDITOR_SHORTCUTS.italic,TextItalic,()=>editor.chain().focus().toggleItalic().run(),state.italic)}
        {action(t('editor.toolbar.strike'),EDITOR_SHORTCUTS.strike,TextStrikethrough,()=>editor.chain().focus().toggleStrike().run(),state.strike)}
      </div>
      <Toolbar.Separator className="editor-tool-separator"/>
      <div className="editor-tool-group">
        {action(t('editor.toolbar.bulletList'),EDITOR_SHORTCUTS.bullet,ListBullets,()=>editor.chain().focus().toggleBulletList().run(),state.bullet)}
        {action(t('editor.toolbar.orderedList'),EDITOR_SHORTCUTS.ordered,ListNumbers,()=>editor.chain().focus().toggleOrderedList().run(),state.ordered)}
        {action(t('editor.toolbar.taskList'),EDITOR_SHORTCUTS.task,ListChecks,()=>editor.chain().focus().toggleTaskList().run(),state.task)}
      </div>
      <Toolbar.Separator className="editor-tool-separator"/>
      <div className="editor-tool-group">
        {action(t('editor.toolbar.quote'),EDITOR_SHORTCUTS.quote,Quotes,()=>editor.chain().focus().toggleBlockquote().run(),state.quote)}
        {action(t('editor.toolbar.codeBlock'),EDITOR_SHORTCUTS.code,Code,()=>editor.chain().focus().toggleCodeBlock().run(),state.code)}
        <Popover.Root open={linkOpen} onOpenChange={open=>{setLinkOpen(open);setError('');if(open)setUrl(editor.getAttributes('link').href||'');}}>
          <Hint label={t('editor.toolbar.link.edit')}><Popover.Trigger asChild><Toolbar.Button type="button" className="editor-tool" aria-label={t('editor.toolbar.link.edit')} aria-pressed={state.link} onMouseDown={e=>e.preventDefault()}><LinkSimple size={18}/></Toolbar.Button></Popover.Trigger></Hint>
          <Popover.Content className="editor-link-popover" sideOffset={9} align="end" onCloseAutoFocus={event=>{event.preventDefault();if(mounted.current && !editor.isDestroyed)editor.commands.focus();}}>
            <div className="editor-link-heading"><label htmlFor="editor-link-url">{t('editor.toolbar.link.address')}</label><Popover.Close asChild><Button type="button" variant="ghost" size="icon-sm" aria-label={t('editor.toolbar.link.close')}><X size={16}/></Button></Popover.Close></div>
            <form onSubmit={event=>{event.preventDefault();applyLink();}}><input id="editor-link-url" className="ui-field" aria-label={t('editor.toolbar.link.address')} value={url} placeholder="https://…" onChange={event=>{setUrl(event.target.value);setError('');}}/><Button type="submit" size="icon" aria-label={t('editor.toolbar.link.apply')}><Check size={18}/></Button></form>
            {error&&<p role="alert">{error}</p>}
            {state.link&&<Button type="button" variant="ghost" size="sm" data-tone="danger" className="editor-link-remove" onClick={()=>{editor.chain().focus().extendMarkRange('link').unsetLink().run();setLinkOpen(false);}}>{t('editor.toolbar.link.remove')}</Button>}
          </Popover.Content>
        </Popover.Root>
      </div>
    </Toolbar.Root>
  </HintGroup>;
}

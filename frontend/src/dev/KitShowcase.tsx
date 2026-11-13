// Showcase of the Humcase kit (development only): each component of `components/ui/`, in
// each state (rest, hover, press, focus, disabled, error), in both themes side by side.
// Reference for the founder and the agents: add here any new kit component or variant.
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import { ArrowRight, FileText, Folder, Lightbulb, ListChecks, Note as StickyNote, Plus, Trash } from '@phosphor-icons/react';
import '../workspace-theme.css';
import '../components/ui/kit.css';
import '../foundations.css';
import '../game-object.css';
import './kit-showcase.css';
import { Button } from '../components/ui/Button';
import { Badge, Count, DueBadge, PriorityBadge, StatusBadge, TagBadge, TypeBadge } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { List, ListRow } from '../components/ui/List';
import { EmptyState } from '../components/ui/EmptyState';
import { Notice, Toast } from '../components/ui/Toast';
import { SegmentedControl } from '../components/ui/SegmentedControl';
import { Chip, ChipGroup } from '../components/ui/Chip';
import { SearchField } from '../components/ui/SearchField';
import { SelectField, TextField } from '../components/ui/Controls';
import { Hint, HintGroup, Kbd } from '../components/ui/Hint';
import { Menu, MenuItem, MenuRadioGroup, MenuSeparator } from '../components/ui/Menu';
import { Dialog, DialogBody, DialogFooter, DialogHeader } from '../components/ui/Dialog';
import { todayLocalIso } from '../utils/datetime';

type State = 'rest' | 'hover' | 'press' | 'focus' | 'disabled' | 'error';
const STATE_ATTR: Partial<Record<State, string>> = { hover: 'data-kit-hover', press: 'data-kit-active', focus: 'data-kit-focus' };

/**
 * Copies the `:hover`, `:active` and `:focus-visible` rules of the loaded stylesheets into rules on
 * attributes (`[data-kit-hover]`…): a state can thus be displayed at rest, without a pointer.
 */
function useForcedStates() {
  useEffect(() => {
    const out: string[] = [];
    const visit = (rules: CSSRuleList, wrap: (css: string) => string) => {
      for (const rule of Array.from(rules)) {
        if (rule instanceof CSSStyleRule && /:(hover|active|focus-visible|focus-within)/.test(rule.selectorText)) {
          const sel = rule.selectorText.replace(/:hover/g, '[data-kit-hover]').replace(/:active/g, '[data-kit-active]')
            .replace(/:focus-visible/g, '[data-kit-focus]').replace(/:focus-within/g, ':has([data-kit-focus])');
          out.push(wrap(`${sel} { ${rule.style.cssText} }`));
        } else if (rule instanceof CSSMediaRule) {
          visit(rule.cssRules, css => wrap(`@media ${rule.conditionText} { ${css} }`));
        }
      }
    };
    const run = () => {
      out.length = 0;
      for (const sheet of Array.from(document.styleSheets)) { try { visit(sheet.cssRules, css => css); } catch { /* external sheet */ } }
      let style = document.getElementById('kit-forced-states') as HTMLStyleElement | null;
      if (!style) { style = document.createElement('style'); style.id = 'kit-forced-states'; document.head.append(style); }
      style.textContent = out.join('\n');
    };
    run();
    const timer = window.setTimeout(run, 400); // stylesheets injected after the first render (Vite)
    return () => window.clearTimeout(timer);
  }, []);
}

/** Sets the state attribute on the target (the element itself or `target` inside it). */
function Forced({ state, target, children }: { state: State; target?: string; children: ReactNode }) {
  const [el, setEl] = useState<HTMLSpanElement | null>(null);
  useEffect(() => {
    const attr = STATE_ATTR[state]; if (!el || !attr) return;
    const node = (target ? el.querySelector(target) : el.firstElementChild) as HTMLElement | null;
    node?.setAttribute(attr, '');
    return () => node?.removeAttribute(attr);
  }, [el, state, target]);
  return <span ref={setEl} className="kit-forced">{children}</span>;
}

function States({ states, target, render }: { states: State[]; target?: string; render: (state: State) => ReactNode }) {
  return <div className="kit-states">{states.map(state => <div key={state} className="kit-state"><span className="kit-state__label">{state}</span><Forced state={state} target={target}>{render(state)}</Forced></div>)}</div>;
}

function Section({ id, title, source, children }: { id: string; title: string; source: string; children: ReactNode }) {
  return <section className="kit-section" id={id}><header><h2>{title}</h2><code>{source}</code></header>{children}</section>;
}

const ALL: State[] = ['rest', 'hover', 'press', 'focus', 'disabled'];
const days = (n: number) => { const d = new Date(); d.setDate(d.getDate() + n); return todayLocalIso(d); };

function Panel({ theme }: { theme: 'dark' | 'light' }) {
  const [segment, setSegment] = useState('all');
  const [dialogOpen, setDialogOpen] = useState(false);
  const [sort, setSort] = useState<'recent' | 'status'>('recent');
  return (
    <div className="workspace-app kit-panel" data-theme={theme}>
      <h1 className="kit-panel__title">{theme === 'dark' ? 'Dark' : 'Light'} theme</h1>

      <Section id="button" title="Button" source="ui/Button.tsx · button.css">
        {(['default', 'secondary', 'ghost', 'field', 'nav', 'link', 'destructive'] as const).map(variant =>
          <div key={variant} className="kit-row"><span className="kit-row__label">{variant}</span>
            <States states={ALL} render={state => <Button variant={variant} disabled={state === 'disabled'}><Plus aria-hidden="true" />Action</Button>} /></div>)}
        <div className="kit-row"><span className="kit-row__label">sizes</span><div className="kit-inline">
          <Button size="xs" variant="ghost">xs</Button><Button size="sm" variant="secondary">sm</Button><Button variant="secondary">default</Button><Button size="lg">lg</Button>
          <Button size="icon-sm" variant="ghost" aria-label="Delete"><Trash /></Button><Button size="icon" variant="secondary" aria-label="Add"><Plus /></Button>
          <Button variant="nav" aria-current="page"><Folder aria-hidden="true" />Open section</Button><Button variant="destructive" data-tone="soft">Soft destructive</Button>
        </div></div>
      </Section>

      <Section id="segmented" title="Segmented control" source="ui/SegmentedControl.tsx · segmented.css">
        <div className="kit-inline">
          <SegmentedControl label="Filter" value={segment} onValueChange={setSegment} items={[{ value: 'all', label: 'All', count: 16 }, { value: 'todo', label: 'To do', count: 12 }, { value: 'done', label: 'Done', count: 2 }]} />
          <SegmentedControl size="sm" label="Small" value={segment} onValueChange={setSegment} items={[{ value: 'all', label: 'All' }, { value: 'todo', label: 'To do' }]} />
        </div>
        <SegmentedControl variant="tabs" label="Tabs" value={segment} onValueChange={setSegment} items={[{ value: 'all', label: 'Whole space' }, { value: 'todo', label: 'Design' }, { value: 'done', label: 'Marketing' }]} />
        <States states={['hover', 'press', 'focus']} target=".ui-segmented__item:not([data-state='on'])" render={() => <SegmentedControl size="sm" label="State" value="a" onValueChange={() => {}} items={[{ value: 'a', label: 'Chosen' }, { value: 'b', label: 'Other' }]} />} />
      </Section>

      <Section id="chips" title="Chips" source="ui/Chip.tsx · chip.css">
        <ChipGroup label="Nature">
          <Chip selected onClick={() => {}}>All natures</Chip>
          <Chip selected={false} data-type="todo" icon={<ListChecks aria-hidden="true" />} count={12} onClick={() => {}}>Task</Chip>
          <Chip selected={false} data-type="idea" icon={<Lightbulb aria-hidden="true" />} count={3} onClick={() => {}}>Idea</Chip>
        </ChipGroup>
        <States states={['hover', 'press', 'focus']} target=".ui-chip" render={() => <Chip selected={false} onClick={() => {}}>Other</Chip>} />
      </Section>

      <Section id="fields" title="Fields" source="ui/Controls.tsx · ui/SearchField.tsx · controls.css">
        <States states={['rest', 'hover', 'focus', 'disabled', 'error']} render={state => <TextField placeholder="Project name" disabled={state === 'disabled'} aria-invalid={state === 'error' || undefined} aria-label="Field" />} />
        <div className="kit-inline">
          <SelectField aria-label="List" defaultValue="b"><option value="a">Work</option><option value="b">Personal</option></SelectField>
          <SelectField aria-label="Compact list" className="ui-field--sm" defaultValue="a"><option value="a">Text</option><option value="b">Heading 1</option></SelectField>
          <SearchField id={`kit-search-${theme}`} label="Search" placeholder="Search a case…" />
        </div>
        <textarea className="ui-field ui-field--prose" aria-label="Writing" defaultValue="Long writing field (report)." />
        <p className="ui-field-error">Error message linked to the field.</p>
      </Section>

      <Section id="menu" title="Menu and tooltip" source="ui/Menu.tsx · ui/Hint.tsx">
        <div className="kit-inline">
          <Menu label="Sort" trigger={<Button variant="field">Sort</Button>}>
            <MenuRadioGroup label="Order" value={sort} onValueChange={setSort} options={[{ value: 'recent', label: 'Recent' }, { value: 'status', label: 'To do first' }]} />
            <MenuSeparator />
            <MenuItem icon={<FileText />} onSelect={() => {}}>Export</MenuItem>
            <MenuItem icon={<Trash />} tone="danger" onSelect={() => {}}>Delete</MenuItem>
          </Menu>
          <HintGroup><Hint label="New case" shortcut="Control+N"><Button variant="secondary">Hover me</Button></Hint></HintGroup>
          <span className="kit-inline"><Kbd>Ctrl</Kbd><Kbd>K</Kbd></span>
        </div>
      </Section>

      <Section id="badge" title="Badges" source="ui/Badge.tsx · badge.css">
        {(['soft', 'outline', 'plain'] as const).map(variant => <div key={variant} className="kit-row"><span className="kit-row__label">{variant}</span><div className="kit-inline">
          {(['neutral', 'accent', 'success', 'warning', 'danger'] as const).map(tone => <Badge key={tone} variant={variant} tone={tone}>{tone}</Badge>)}</div></div>)}
        <div className="kit-inline">
          <StatusBadge status="open" /><StatusBadge status="in_progress" /><StatusBadge status="resolved" /><StatusBadge status="in_progress" iconOnly />
          <PriorityBadge priority="P0" status="open" /><PriorityBadge priority="P1" status="open" /><PriorityBadge priority="P2" status="open" /><PriorityBadge priority="P0" status="resolved" />
          <TypeBadge type="bug" /><TypeBadge type="idea" size="md" />
          <DueBadge due={days(-1)} /><DueBadge due={days(0)} /><DueBadge due={days(3)} />
          <Count value={12} /><TagBadge tag="site" />
        </div>
        <States states={['rest', 'hover', 'focus']} render={() => <TagBadge tag="clickable" onClick={() => {}} label="Filter by tag" />} />
      </Section>

      <Section id="card" title="Card" source="ui/Card.tsx · card.css">
        <States states={['rest', 'hover', 'focus']} render={() => <Card interactive className="kit-card"><strong>Interactive card</strong><p>Raised on hover, focus ring.</p></Card>} />
        <div className="kit-inline kit-cards">
          <Card active className="kit-card"><strong>Selected</strong><p>Open case.</p></Card>
          <Card tone="sunken" className="kit-card"><strong>Tray</strong><p>Board column.</p></Card>
          <Card tone="dashed" className="kit-card"><strong>Invitation</strong><p>Create something.</p></Card>
          <span className="ui-icon-tile"><FileText size={18} /></span>
          <span className="ui-icon-tile" data-tinted style={{ '--ui-tile-tint': '#2f8f7a' } as CSSProperties}><Folder size={20} /></span>
        </div>
      </Section>

      <Section id="list" title="List and row" source="ui/List.tsx · card.css">
        <List label="Boxed list">
          <ListRow title="Proofread the pricing page" onClick={() => {}} actionLabel="Open Proofread the pricing page" leading={<span className="ui-icon-tile"><FileText size={18} /></span>}
            description="The FAQ section contradicts the banner about the trial period." meta={<><StatusBadge status="open" /><span>Campaign · 2 h ago</span></>} trailing={<ArrowRight size={16} />} />
          <ListRow title="Selected row" active onClick={() => {}} actionLabel="Open the selected row" overline={<><StatusBadge status="in_progress" /><PriorityBadge priority="P1" status="in_progress" /></>} meta="edited yesterday">
            <span className="kit-inline"><TagBadge tag="tag" onClick={() => {}} label="Filter by tag" /></span>
          </ListRow>
        </List>
        <States states={['hover', 'focus']} target=".ui-list-row__action" render={() => <List variant="divided"><ListRow size="sm" title="Row to continue" onClick={() => {}} actionLabel="Continue" leading={<StatusBadge status="in_progress" iconOnly />} meta="Project" trailing={<ArrowRight size={15} />} /></List>} />
      </Section>

      <Section id="feedback" title="Empty states and messages" source="ui/EmptyState.tsx · ui/Toast.tsx">
        <EmptyState icon={<StickyNote />} title="No case yet" description="Your ideas, tasks and discoveries will appear here." action={<Button variant="secondary"><Plus aria-hidden="true" />Create a case</Button>} />
        <EmptyState size="sm" description="Drop a card here" />
        <div className="kit-toasts">
          <Toast duration={8000} action={<Button variant="link" size="sm">Undo</Button>}>“Brief” deleted</Toast>
          <Toast tone="success">Archive saved</Toast>
          <Toast tone="danger">Cannot read the data: retrying automatically…</Toast>
        </div>
        <div className="kit-inline kit-notices"><Notice live={false}>Valid archive.</Notice><Notice live={false} tone="success">Key saved.</Notice><Notice live={false} tone="danger">Moving the card failed.</Notice></div>
      </Section>

      <Section id="dialog" title="Dialog" source="ui/Dialog.tsx · dialog.css">
        <div className="kit-inline"><Button variant="secondary" onClick={() => setDialogOpen(true)}>Open a dialog</Button></div>
        {dialogOpen && <Dialog size="sm" onClose={() => setDialogOpen(false)} describedBy={`kit-dialog-${theme}`}>
          <form className="ui-dialog__form" onSubmit={event => { event.preventDefault(); setDialogOpen(false); }}>
            <DialogHeader title="New project" description={<p id={`kit-dialog-${theme}`}>Escape, button or Cancel all go through onClose.</p>} onClose={() => setDialogOpen(false)} />
            <DialogBody><label className="ui-dialog__field">Name<TextField autoFocus placeholder="Back-to-school campaign" /></label><div role="alert" className="ui-dialog__error">Example dialog error.</div></DialogBody>
            <DialogFooter note="Optional footer note."><Button type="button" variant="secondary" onClick={() => setDialogOpen(false)}>Cancel</Button><Button type="submit">Create</Button></DialogFooter>
          </form>
        </Dialog>}
      </Section>
    </div>
  );
}

export function KitShowcase() {
  useForcedStates();
  return <main className="kit-showcase"><Panel theme="dark" /><Panel theme="light" /></main>;
}

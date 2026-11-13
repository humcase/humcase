# Humcase design system — reference

**Frozen system**: the kit inventory is complete and every screen is built from it. "Frozen" means stable and
complete, **not aesthetically validated**: palette, composition and component choices remain revisable. Evolve the
kit by extending its components and tokens rather than restyling screens; tasks and evidence stay in Humcase.

## Product intent

The workspace lets you recover context, prepare a task, keep writing during external work, review a result and its
sources, then accept it or ask for a redo. Writing stays central. The Work/Personal profiles, activities, metadata, history and links do not disappear in favor of
presentation. A declared status does not mean an agent is observed running; the product
remains useful without AI.

## Where the rules live

| Layer | File | Content |
| --- | --- | --- |
| Tokens | `frontend/src/workspace-theme.css` | Colors of both themes, typography, spacing, radii, shadows, curves; bridge to the shadcn variables (`--accent`, `--card`, `--sidebar-*`) |
| Foundations | `frontend/src/foundations.css`, `game-object.css` | Cross-cutting rules (focus, selection, icon sizes) and the "game object" universe |
| Kit | `frontend/src/components/ui/` | React components + one CSS sheet each, imported in order by `kit.css` |
| Primitives | `frontend/src/components/shadcn/` | shadcn/ui on Radix (sidebar, popover, collapsible…), installed by `npx shadcn add` |
| Screens | `workspace-*.css`, `components/**/*.css` | **Layout only**: grids, columns, margins, widths |

Ownership rule: surfaces, colors, borders, typography and interactive states belong to the kit. The kit's
default dimensions (minimum width, list margins, padding) are written in `:where()` so
that a screen can set its layout without fighting specificity. A screen sheet never redefines
the color or typography of a kit component.

## Foundations

### "Game object" universe

The app presents itself as an object: silk-screened labels (DM Mono 11.5 px, capitals, 0.09 em spacing)
for dates, sections and organization labels; mono digits; keys with a face and an edge
(`--key-edge`, `--key-action-edge`) that sink 2 px when pressed; "Your day" as a screen
(`--screen-bg`, `--screen-dot` dot pattern) with 12 × 12 pixel icons (`PixelIcon`). Counters off the screen are
small recessed displays (`Count`). Body text stays in DM Sans.

### Colors

A single temperature: slightly warm neutrals, orange accent (`--workspace-action`: copper orange #E78A53 in dark, burnt orange #C4410C in light). Four depth levels carried by
tint and shadow: frame (sidebar) < page < card < raised element (menus, popovers, dialogs). In light,
cards are true white like raised elements and stand out from the page by a firm line and a shadow
that reads (02/10/2026: page and cards nearly the same white melted together on a bright screen).

| Role | Token | Light | Dark |
| --- | --- | --- | --- |
| Frame, sidebar | `--workspace-frame` / `--workspace-sidebar-bg` | #F1F0EF | #0F0F0E |
| Page | `--workspace-canvas` | #F7F6F4 | #161615 |
| Card | `--workspace-surface` | #FFFFFF | #1D1D1B |
| Menu, popover, dialog | `--workspace-raised` | #FFFFFF | #252523 |
| Subtle fill | `--workspace-muted-surface` | #F1F0EF | #262624 |
| Main text | `--workspace-ink` | #21201C | #EDEDEA |
| Secondary text | `--workspace-text-secondary` | #3B3A36 | #B3B0AA |
| Subdued text | `--workspace-text-muted` | #63635E | #8F8C86 |
| Hairline | `--workspace-line` | #DCDBD6 | #2B2A28 |
| Card border | `--workspace-card-border` | #BAB9B2 | #363531 |
| Card border, pointed at | `--workspace-card-hover-border` | #84837C | #45433F |
| Action, focus | `--workspace-action` | #C4410C | #E78A53 |
| Success | `--workspace-success` | #44664F | #ABCDB6 |
| Warning (text) | `--workspace-warning-ink` | #8F5514 | #F0C48D |
| Danger | `--workspace-danger` | #A43939 | #F0A8A8 |
| Dialog overlay | `--workspace-overlay` | #21201C47 | #0A0A09B3 |

Interactive states: a single scale, `--workspace-state-hover`, `--workspace-state-pressed`,
`--workspace-state-selected`, connected to shadcn through `--accent` and `--sidebar-accent`. Shadows:
`--workspace-shadow-card`, `-card-hover`, `-raised`, `-panel`, `-dialog`.

**Reserved accent**: orange marks the page's primary action, the timeline's current time and
agent writes. Statuses, P2/P3 priorities, filters, view controls and active navigation are neutral;
only the status icon carries its semantic color; P0 and P1 keep red and amber.

### Logo

An orange key screen-printed "h" (DM Mono 500), raised on a short shadow, with its indicator light, framed to fill its square like other apps' icons; the same drawing at every size and on every ground, so its colors never follow the theme. Source: `src-tauri/icons/source.svg` (identical to `site/public/favicon.svg`), drawn in the app by `components/brand/HumcaseKey.tsx`. The app icons use a variant framed edge to edge without the shadow (`src-tauri/icons/source-icon.svg`), so the key is as large as its neighbours in the taskbar (24 px at 100 %), from its 1024 px render: `npx tauri icon src-tauri/icons/source-1024.png -o src-tauri/icons` (then remove the generated `android/`, `ios/` and `64x64.png`, which the app does not use). The wordmark is HUMCASE in DM Mono, HUM in copper.

### Typography, spacing, radii, motion

| Role | Token | Value |
| --- | --- | --- |
| Caption, metadata | `--workspace-text-caption` | 12.5 px |
| Control, navigation | `--workspace-text-ui` | 14 px |
| Document body | `--workspace-text-body` | 16 px, line height 1.65 |
| Card title | `--workspace-text-card` | 16 px, weight 600 |
| Page title | `--workspace-text-display` | 30 px, weight 600, −0.025 em |

Fonts: DM Sans (interface), DM Mono (labels, digits, shortcuts). Weights 400 text, 500 interface,
600 titles. Spacing: `--workspace-space-1…8` = 4, 8, 12, 16, 20, 24, 32 px. Radii: 6 px controls
(`--workspace-radius-control`), 12 px cards, groups and dialogs. Controls: 40 px standard, 32 px compact,
44 px under touch pointer. Motion: `--workspace-ease-out` (cubic-bezier(.23, 1, .32, 1)), 140 to 200 ms,
press `scale(.97)`; everything is disabled under `prefers-reduced-motion` by `ui/motion.css` (except loading
indicators). The application is displayed with a native WebView zoom according to the screen's resolution (since 0.9.56).

## Kit inventory

| Component | Base | Variants and usage |
| --- | --- | --- |
| `Button` | Origin UI Button (21st 143), cva + Slot | `default` (orange action key), `secondary`/`outline` (neutral key), `field` (select trigger), `ghost` (discreet command, `data-tone="danger"`), `nav` (navigation entry, `aria-current`), `link`, `destructive`; sizes `xs`, `sm`, standard, `lg`, `icon`, `icon-sm` |
| `SegmentedControl` | Radix ToggleGroup | `track` (views, filters) or `tabs` (activities), `size="sm"`, optional counter |
| `Chip`, `ChipGroup` | — | A filter chosen in one click and cleared by a second (`aria-pressed`); the chosen chip is a raised key like the active segment; `data-type` tints its icon with the nature; 32 px high |
| `Menu`, `MenuItem`, `MenuRadioGroup`, `MenuSeparator` | Radix DropdownMenu (Origin UI 383) | Sort, status, frames; portal into `.workspace-app` to inherit the theme |
| `Hint`, `HintGroup`, `Kbd` | Radix Tooltip | Name of an icon control and its shortcut as keys |
| `SearchField`, `TextField`, `SelectField`, `.ui-field` | native fields | `--sm` variant, `textarea.ui-field--prose`; error through `aria-invalid="true"`; open list drawn like `Menu` (customizable select, `appearance: base-select`), except in quick capture |
| `Dialog`, `DialogHeader`, `DialogBody`, `DialogFooter`, `DialogTitle` | Radix Dialog (Origin UI 365) | Sizes `sm`, `md`, `lg`, `xl`, `page`, `palette`; trapped focus, Escape, focus return |
| `Badge`, `Count`, `StatusBadge`, `PriorityBadge`, `TypeBadge`, `DueBadge`, `TagBadge` | — | Tones `neutral`, `danger`, `warning`, `success`, `accent`; styles `soft`, `outline`, `plain`, `screen` |
| `Card` | — | Tones `default`, `sunken`, `dashed`; padding `none`→`lg`; `interactive`, `active` |
| `List`, `ListRow` | — | `boxed`, `divided`, `plain`; row with overline, description, meta, leading and trailing elements, accessible stretched action; tinted icon tiles `.ui-icon-tile[data-tinted]` |
| `EmptyState` | Empty (cnippet, 21st 19749) | `md` with fanned-out keys, `sm` compact; icon, title, description, action |
| `Toast`, `Notice` | Undo Pill (21st 29941) | Tones `neutral`, `success`, `danger`; action (Undo), remaining-time ring; `Notice` for an inline message |
| `PixelIcon` | — | 12 × 12 glyphs of the "Your day" screen |

Phosphor is the only icon family. A clickable element left as a native `<button>` is a surface (card,
column, resume item), never a command.

## Screen compositions

These blocks are specific to a screen but **assembled only from kit primitives**; they define neither
their own color nor typography:

- Page header (`workspace/PageHeader.tsx`): eyebrow, title with its count beside it, a line of context, and the page action on the right (`NewCaseButton`, "New case" with Ctrl+N wherever a case can be created). Every page uses it: Home, a project or All cases (with the view switch beside "New case", in all four views), Today, the Inbox, the shortcuts, the trash. Spacing from the scale only; no page redefines it.

- "Your day" timeline (`DayTimeline`): adaptation of [Agent Trace](https://21st.dev/@n1m4mz/components/agent-trace), shadcn Popover, `Button xs`.
- Linked tasks (`LinkedTaskPlan`): adaptation of [AI Task List](https://21st.dev/@educalvolpz/components/ai-task-list) and [Agent Plan](https://21st.dev/@isaiahbjork/components/agent-plan), persisted Humcase data.
- Note history (`NoteHistory`), report, links (`NoteRelations`): `List`, `Badge`, `EmptyState`.
- Sidebar: shadcn Sidebar and Collapsible ([Dashboard Sidebar](https://21st.dev/@arunjdass/components/dashboard-sidebar)).
- Settings: `Dialog page`, `Button nav`, `Card`, `Notice` ([Settings — LN](https://21st.dev/@ln-dev7/components/settings)).
- Editor toolbar: `Button ghost icon-sm`, `HintGroup`, `.ui-field--sm`.

## Kit showcase

`frontend/kit.html` shows each component in each state — rest, hover, press, focus, disabled,
error — in both themes side by side. States are forced by copying the kit's `:hover`, `:active`
and `:focus-visible` rules onto `data-kit-*` attributes: the showcase contains no component styles.

```bash
cd frontend
npm run dev
```

Then open `http://localhost:5173/kit.html`. The showcase exists only in development: it is not an
entry of `build.rollupOptions.input`, its entry point refuses to start outside `import.meta.env.DEV`, and
`src/dev/kit-showcase.test.ts` fails if it enters the build.

## Accessibility (as measured at 0.9.68)

> Measured before the canvas of cases, the sketch editor and the light theme of 02/10/2026 (firmer card lines): to measure again on those screens.

Measurements on an isolated profile, two themes, all screens (home, cards, list, Board, Today,
Inbox, note page and panels, settings and sub-pages, palette, dialogs, quick capture, mini,
first launch with no note):

- Text contrast ≥ 4.5:1: no failures (only exception: the logotype, exempt as a brand mark).
- Keyboard focus: every tab stop has a visible indicator; logical order; dialogs trapping focus.
- `prefers-reduced-motion`: 55 active animations → 0 (only the loading indicator spins).
- Widths 720, 1440, 1920, 2560 px: no horizontal overflow.

Colors never carry meaning alone (statuses: icon + label). Reference: [W3C Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html).

## Structure and adaptation

Stable navigation; one page title; an identifiable
primary action; writing and context separated; note data accessible without an intermediate step.
The tracking panel moves below the document when width runs short. Document content keeps a reading width
of 65 to 78 ch. The 720/1440/1920/2560 px captures, the two themes and mini are check
points, not independent layouts.

**One page frame.** Every page of the space (Home, cases, a project, Today, the shortcuts by nature, Inbox,
Trash) sits in `.workspace-page`: full width between the two gutters (`--workspace-page-gutter`: 36 px,
56 from 1600 px, 72 from 2240 px, 20 under 760 px), top and bottom from `--workspace-page-top` and
`--workspace-page-bottom`. A page never sets its own width, gutters or top; a reading width (in `ch`)
belongs to the text inside, not to the page. On the cases page the view bar, the filter bar and the view
start on that same gutter. `page-frame.test.ts` fails if a page class sets a width, a margin or a padding.

## Known limits

- A few raw colors remain in `editor-toolbar.css` as fallback values or shadows.
- Tailwind classes tied to the shadcn tokens (`text-muted-foreground`, `bg-card`) remain in some screens; they follow the theme.
- The `prose-invert` block of `workspace-theme.css` is still used by the Tiptap editor.

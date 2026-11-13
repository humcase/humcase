import { describe, expect, it } from 'vitest';
import { cssRules, readSource as read, sourceFiles } from '../../test/cssSources';

const files = sourceFiles();

// The page frame (width, gutters, top and bottom) is set once on `.workspace-page`.
const FRAME = /^\s*(max-width|width|margin(-inline|-left|-right)?|padding(-top|-bottom|-inline|-left|-right)?)\s*:/m;

/** Classes given to a page next to `workspace-page` (`workspace-home`, `nature-view`...). */
function pageClasses(): string[] {
  const classes = new Set<string>();
  for (const file of files.filter(f => f.endsWith('.tsx') && !f.endsWith('.test.tsx'))) {
    for (const [, names] of read(file).matchAll(/className="([^"]*\bworkspace-page\b[^"]*)"/g)) {
      for (const name of names.split(/\s+/)) if (name && name !== 'workspace-page') classes.add(name);
    }
  }
  return [...classes];
}

describe('Page frame', () => {
  it('finds the pages built on .workspace-page', () => {
    expect(pageClasses()).toEqual(expect.arrayContaining(['workspace-home', 'nature-view', 'workspace-notes']));
  });

  it('no page sets its own width, gutters or top: every page has the same frame', () => {
    const pages = pageClasses();
    const offenders: string[] = [];
    for (const file of files.filter(f => f.endsWith('.css'))) {
      for (const { selector, declarations: block } of cssRules(read(file))) {
        const declarations = block.replace(/;/g, ';\n');
        for (const part of selector.split(',')) {
          const target = part.trim().split(/[\s>+~]+/).pop() ?? '';
          const page = pages.find(name => new RegExp(`\\.${name}(?![\\w-])`).test(target));
          if (page && FRAME.test(declarations)) offenders.push(`${file}: ${part.trim()}`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });
});

// Review of lot C (03/10): the narrow-window rule putting the natures on one scrolling line lost
// to the kit's `.workspace-app .ui-chips { flex-wrap: wrap }` and never applied.
describe('Narrow window', () => {
  const classes = (selector: string) => (selector.match(/\.[\w-]+/g) ?? []).length;
  it("the natures scroll on one line: the rule outweighs the kit's chips", () => {
    const kit = cssRules(read('components/ui/chip.css')).filter(rule => /(^|\s)\.ui-chips$/.test(rule.selector.trim()) && /flex-wrap/.test(rule.declarations));
    const narrow = cssRules(read('workspace-shell.css')).filter(rule => rule.selector.includes('case-filter-bar__natures') && /flex-wrap:\s*nowrap/.test(rule.declarations));
    expect(kit.length).toBeGreaterThan(0);
    expect(narrow.length).toBe(1);
    for (const rule of kit) expect(classes(narrow[0].selector)).toBeGreaterThan(classes(rule.selector));
  });
});

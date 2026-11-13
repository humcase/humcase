import { describe, expect, it } from 'vitest';
import { cssRules, readSource, sourceFiles } from '../../test/cssSources';

// Properties that draw a component: they belong to the kit (components/ui/).
const APPEARANCE = /^(color|background(-color|-image)?|border(-color|-top|-right|-bottom|-left)?|font-(size|family|weight)|box-shadow)$/;

/** True if the final target of a selector is a kit component (`.ui-…`). */
function kitTarget(selector: string): boolean {
  return selector.split(',').some(part => {
    const last = part.trim().split(/\s+|>|\+|~/).filter(Boolean).pop() ?? '';
    // Pseudo-elements (::placeholder, ::picker) describe the inside: same rule.
    return /\.ui-[\w-]+/.test(last.replace(/:not\([^)]*\)/g, ''));
  });
}

// Review 0.9.69 (#6): screen stylesheets were repainting kit components (background, font,
// border), so the same button or field changed appearance from one screen to another.
describe('Kit — no screen stylesheet repaints a kit component', () => {
  it('stylesheets outside components/ui set no color, background, border or font on .ui-*', () => {
    const files = sourceFiles()
      .filter(f => f.endsWith('.css') && !f.startsWith('components/ui/'));
    expect(files.length).toBeGreaterThan(10);
    const offenders: string[] = [];
    for (const file of files) {
      for (const { selector, declarations } of cssRules(readSource(file))) {
        if (!kitTarget(selector)) continue;
        for (const declaration of declarations.split(';')) {
          const property = declaration.split(':')[0]?.trim() ?? '';
          if (APPEARANCE.test(property)) offenders.push(`${file} — ${selector} { ${declaration.trim()} }`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  // A screen class placed on a kit component (`<Card className="…">`, `className="ui-field x"`)
  // targets it just as much as `.ui-*`: it may set the layout, not repaint the component.
  // The color and font of the contained text remain allowed on a container.
  it('screen classes placed on a kit component repaint no background, border or shadow', () => {
    const kitComponents = new Set<string>();
    const sources = sourceFiles();
    for (const file of sources.filter(f => /^components\/ui\/[^/]+\.tsx$/.test(f) && !f.endsWith('.test.tsx'))) {
      for (const [, fileName] of readSource(file).matchAll(/export (?:function|const) ([A-Z]\w*)/g)) kitComponents.add(fileName);
    }
    const kitClasses = new Set<string>();
    for (const file of sources.filter(f => f.endsWith('.tsx') && !f.startsWith('components/ui/') && !f.endsWith('.test.tsx'))) {
      const code = readSource(file);
      for (const [, tag, classes] of code.matchAll(/<([A-Z]\w*)\b[^>]*?className="([^"]+)"/g)) {
        if (kitComponents.has(tag)) classes.split(/\s+/).forEach(c => kitClasses.add(c));
      }
      for (const [, classes] of code.matchAll(/className="([^"]*\bui-[^"]*)"/g)) {
        classes.split(/\s+/).filter(c => !c.startsWith('ui-')).forEach(c => kitClasses.add(c));
      }
    }
    expect(kitClasses.has('editor-block-select')).toBe(true);
    // Accepted exception: quick capture is drawn like the object's screen ("game object" universe).
    const exceptions = new Set(['quick-capture__input']);
    const paint = /^(background(-color|-image)?|border(-color|-top|-right|-bottom|-left)?|box-shadow)$/;
    const escapeRegex = (c: string) => c.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const offenders: string[] = [];
    for (const file of sources.filter(f => f.endsWith('.css') && !f.startsWith('components/ui/'))) {
      for (const { selector, declarations } of cssRules(readSource(file))) {
        const target = selector.split(',').some(part => {
          const last = (part.trim().split(/\s+|>|\+|~/).filter(Boolean).pop() ?? '').replace(/:not\([^)]*\)/g, '');
          return [...kitClasses].some(c => !exceptions.has(c) && new RegExp(`\\.${escapeRegex(c)}(?![\\w-])`).test(last));
        });
        if (!target) continue;
        for (const declaration of declarations.split(';')) {
          if (paint.test(declaration.split(':')[0]?.trim() ?? '')) offenders.push(`${file} — ${selector} { ${declaration.trim()} }`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });
});

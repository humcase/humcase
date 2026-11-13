import { describe, expect, it } from 'vitest';
import { cssRules, readSource as read } from '../../test/cssSources';

const controls = read('components/ui/controls.css');
const editorToolbar = read('components/editor-toolbar.css');
const mini = read('workspace-mini.css');

// Open lists are drawn by the kit (customizable select). A screen stylesheet that
// puts `appearance: none` back on a kit list brings back the system's white menu with a blue hover.
describe('Kit lists — open menu drawn by the kit', () => {
  it('the kit enables the customizable select and draws the menu', () => {
    expect(controls).toMatch(/select\.ui-field::picker\(select\)\s*\{\s*appearance:\s*base-select/);
    expect(controls).toMatch(/::picker\(select\)\s*\{[^}]*var\(--workspace-raised\)/);
  });

  it('no screen stylesheet gives its native appearance back to a kit list', () => {
    for (const css of [editorToolbar, mini]) {
      const offending = cssRules(css).filter(({ selector, declarations }) => /appearance:\s*none/.test(declarations) && /(editor-block-select|select(?!:not\(\.ui-field\)))/.test(selector) && !/select:not\(\.ui-field\)/.test(selector));
      expect(offending).toEqual([]);
    }
  });
});

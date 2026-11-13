import { afterEach, describe, expect, it } from 'vitest';
import { render, fireEvent, screen, cleanup, act } from '@testing-library/react';
import { Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { TaskItem, TaskList } from '@tiptap/extension-list';
import { Markdown } from 'tiptap-markdown';
import { EditorToolbar } from './EditorToolbar';
import { EDITOR_SHORTCUTS } from './editorShortcuts';
let editor: Editor;
afterEach(()=>{cleanup();editor?.destroy();});
function setup(){
 editor=new Editor({extensions:[StarterKit,TaskList,TaskItem,Markdown],content:'Un texte à conserver'});
 render(<EditorToolbar editor={editor}/>);
 act(()=>{editor.commands.setTextSelection({from:1,to:9});});
}
describe('Formatting toolbar connected to the document',()=>{
 it('applies bold to the selected text and keeps the text in the Markdown',()=>{
  setup();fireEvent.click(screen.getByRole('button',{name:'Gras'}));
  expect(editor.isActive('bold')).toBe(true);
  expect((editor.storage as any).markdown.getMarkdown()).toContain('**Un texte**');
 });
 it('creates a heading then allows going back to a paragraph without losing the text',()=>{
  setup();fireEvent.change(screen.getByLabelText('Style du paragraphe'),{target:{value:'2'}});
  expect(editor.isActive('heading',{level:2})).toBe(true);
  fireEvent.change(screen.getByLabelText('Style du paragraphe'),{target:{value:'0'}});
  expect(editor.getText().trim()).toBe('Un texte à conserver');expect(editor.isActive('paragraph')).toBe(true);
 });
 it('creates a checklist compatible with the existing serialization',()=>{
  setup();fireEvent.click(screen.getByRole('button',{name:'Liste de tâches'}));
  expect(editor.isActive('taskList')).toBe(true);
  expect((editor.storage as any).markdown.getMarkdown()).toContain('[ ] Un texte à conserver');
 });
 it('refuses an executable link and applies an HTTPS address on the selection',()=>{
  setup();fireEvent.click(screen.getByRole('button',{name:'Modifier le lien'}));
  fireEvent.change(screen.getByRole('textbox',{name:'Adresse du lien'}),{target:{value:'javascript:alert(1)'}});
  fireEvent.click(screen.getByRole('button',{name:'Appliquer le lien'}));
  expect(screen.getByRole('alert').textContent).toContain('https://');
  expect(editor.isActive('link')).toBe(false);
  fireEvent.change(screen.getByRole('textbox',{name:'Adresse du lien'}),{target:{value:'https://example.com'}});
  fireEvent.click(screen.getByRole('button',{name:'Appliquer le lien'}));
  expect(editor.getAttributes('link').href).toBe('https://example.com');
  expect((editor.storage as any).markdown.getMarkdown()).toContain('[Un texte](https://example.com)');
 });

});

describe('Shortcuts shown in the tooltips',()=>{
 // The tooltips announce these shortcuts: each must actually produce the described effect.
 const tiptap=(shortcut:string)=>shortcut.replace('Control','Mod').replace(/\+/g,'-').replace(/-([A-Z])$/,(_m,k:string)=>'-'+k.toLowerCase());
 const run=(shortcut:string)=>act(()=>{editor.commands.keyboardShortcut(tiptap(shortcut));});
 it.each([
  ['bold',()=>editor.isActive('bold')],['italic',()=>editor.isActive('italic')],['strike',()=>editor.isActive('strike')],
  ['bullet',()=>editor.isActive('bulletList')],['ordered',()=>editor.isActive('orderedList')],['task',()=>editor.isActive('taskList')],
  ['quote',()=>editor.isActive('blockquote')],['code',()=>editor.isActive('codeBlock')],
 ] as const)('%s: the announced shortcut applies the formatting',(key,isOn)=>{
  setup();run(EDITOR_SHORTCUTS[key]);expect(isOn()).toBe(true);
 });
 it('undo then redo with the keyboard',()=>{
  setup();run(EDITOR_SHORTCUTS.bold);expect(editor.isActive('bold')).toBe(true);
  run(EDITOR_SHORTCUTS.undo);expect(editor.isActive('bold')).toBe(false);
  run(EDITOR_SHORTCUTS.redo);expect(editor.isActive('bold')).toBe(true);
 });
 it("exposes the shortcut to assistive technologies",()=>{
  setup();expect(screen.getByRole('button',{name:'Gras'}).getAttribute('aria-keyshortcuts')).toBe('Control+B');
 });
});

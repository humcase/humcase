import { describe, expect, it } from 'vitest';
// Non-literal specifier: Node types stay out of the application's compilation.
const fsModule = 'node:fs';
type Fs = {
  readFileSync: (path: string, encoding: 'utf8') => string;
  readdirSync: (path: string) => string[];
};
const { readFileSync, readdirSync } = (await import(/* @vite-ignore */ fsModule)) as Fs;

const COMMANDS_DIR = '../src-tauri/src/commands';
// Arguments Tauri fills itself: the interface never sends them.
const INJECTED = /\b(State|AppHandle|Window|WebviewWindow|Webview)\b/;

type Param = { name: string; optional: boolean };

/** Splits on commas outside `<…>`, `(…)` and `[…]`. */
function splitTopLevel(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ('<([{'.includes(c)) depth++;
    else if ('>)]}'.includes(c)) depth--;
    else if (c === ',' && depth === 0) { parts.push(text.slice(start, i)); start = i + 1; }
  }
  parts.push(text.slice(start));
  return parts.map(p => p.replace(/\/\/[^\n]*/g, '').trim()).filter(Boolean);
}

/** Rust commands and the arguments the interface must or may send (snake_case). */
function rustCommands(): Map<string, Param[]> {
  const commands = new Map<string, Param[]>();
  for (const file of readdirSync(COMMANDS_DIR).filter(f => f.endsWith('.rs'))) {
    const code = readFileSync(`${COMMANDS_DIR}/${file}`, 'utf8');
    for (const [, name, params] of code.matchAll(/#\[tauri::command\]\s*(?:#\[[^\]]*\]\s*)*pub\s+(?:async\s+)?fn\s+(\w+)(?:<[^>]*>)?\s*\(([\s\S]*?)\)\s*(?:->|\{)/g)) {
      commands.set(name, splitTopLevel(params)
        .map(p => {
          const [, param, type] = /^(?:mut\s+)?(\w+)\s*:\s*([\s\S]+)$/.exec(p) ?? [];
          return { param, type };
        })
        .filter(({ param, type }) => param && !INJECTED.test(type))
        .map(({ param, type }) => ({ name: param.replace(/^_/, ''), optional: /^Option\s*</.test(type) })));
    }
  }
  return commands;
}

const snake = (key: string) => key.replace(/[A-Z]/g, c => `_${c.toLowerCase()}`);

/** Every `invoke('command', { … })` of the interface, with the keys of its arguments. */
function invokeCalls(): { command: string; keys: string[] }[] {
  const calls: { command: string; keys: string[] }[] = [];
  for (const file of ['src/services/tauriApi.ts', 'src/main.tsx']) {
    const code = readFileSync(file, 'utf8');
    for (const [, command, args] of code.matchAll(/invoke(?:<[^>]*>)?\(\s*'(\w+)'\s*(?:,\s*\{([^}]*)\})?\s*\)/g)) {
      calls.push({ command, keys: args === undefined ? [] : splitTopLevel(args).map(a => a.split(':')[0].trim()) });
    }
  }
  return calls;
}

// Tauri matches the keys sent by the interface (camelCase) to the Rust parameter names
// (snake_case). A key that matches nothing is dropped silently: `titreAuto` sent to a command
// expecting `auto_title` meant a dictation appended in the background never got its title.
describe('Tauri commands — what the interface sends is what Rust reads', () => {
  const commands = rustCommands();
  const calls = invokeCalls();

  it('reads the commands on both sides', () => {
    expect(commands.size).toBeGreaterThan(50);
    expect(calls.length).toBeGreaterThan(50);
    expect(commands.get('append_note_content')?.map(p => p.name)).toEqual(['id', 'text', 'auto_title']);
  });

  it('every call names an existing command, with keys Rust reads and every required argument', () => {
    const problems: string[] = [];
    for (const { command, keys } of calls) {
      const params = commands.get(command);
      if (!params) { problems.push(`${command}: no such command`); continue; }
      const sent = new Set(keys.map(snake));
      for (const key of keys) {
        if (!params.some(p => p.name === snake(key))) problems.push(`${command}: "${key}" matches no parameter`);
      }
      for (const p of params) {
        if (!p.optional && !sent.has(p.name)) problems.push(`${command}: required "${p.name}" is not sent`);
      }
    }
    expect(problems).toEqual([]);
  });
});

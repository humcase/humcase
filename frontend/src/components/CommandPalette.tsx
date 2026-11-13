import { useEffect, useRef, useState } from 'react';
import { Command } from 'cmdk';
import { useTranslation } from 'react-i18next';
import { MagnifyingGlass as Search, List, SlidersHorizontal as Settings } from '@phosphor-icons/react';
import { EmptyState } from './ui/EmptyState';
import type { Note, Project } from '../types';
import { Dialog } from './ui/Dialog';
import { Kbd } from './ui/Hint';
import { Button } from './ui/Button';
import './workspace/command-palette.css';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  projects: Project[];
  notes: Note[];
  onSelectProject: (id: string | null) => void;
  onSelectNote: (id: string) => void;
  onOpenSettings: () => void;
}

export default function CommandPalette({ isOpen, onClose, projects, notes, onSelectProject, onSelectNote, onOpenSettings }: CommandPaletteProps) {
  const { t } = useTranslation();
  // Controlled search: the empty state offers to clear it (no keyboard dead end). Reset
  // on every opening, during render.
  const [search, setSearch] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const [wasOpen, setWasOpen] = useState(isOpen);
  if (wasOpen !== isOpen) {
    setWasOpen(isOpen);
    if (isOpen) setSearch('');
  }
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        if (isOpen) {
          onClose();
        } else {
          document.dispatchEvent(new CustomEvent('open-command-palette'));
        }
      }
    };
    document.addEventListener('keydown', down);
    return () => document.removeEventListener('keydown', down);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  // Kit dialog, "palette" format: Escape and a click outside close it.
  return (
    <Dialog onClose={onClose} size="palette" label={t('notes.palette.label')} closeOnOutside className="workspace-command-dialog">
        <Command className="w-full flex flex-col" label={t('notes.palette.label')}>
          <div className="flex items-center border-b border-border px-3">
            <Search size={20} aria-hidden="true" className="mr-2" />
            <Command.Input
              ref={inputRef}
              value={search}
              onValueChange={setSearch}
              placeholder={t('notes.palette.placeholder')}
              className="w-full bg-transparent border-none focus:outline-none"
            />
          </div>

          <Command.List className="workspace-command-results overflow-y-auto p-2 scrollbar-thin">
            <Command.Empty><EmptyState size="sm" description={t('notes.palette.noResult')} action={<Button type="button" variant="secondary" size="sm" onClick={() => { setSearch(''); inputRef.current?.focus(); }}>{t('notes.palette.clearSearch')}</Button>} /></Command.Empty>

            <Command.Group heading={t('notes.palette.projects')} className="mb-2">
              <Command.Item 
                onSelect={() => { onSelectProject(null); onClose(); }}
                className="flex items-center gap-2 px-2 py-2.5 rounded-md cursor-pointer"
              >
                <List className="w-4 h-4" /> {t('notes.palette.allNotes')}
              </Command.Item>
              {projects.map(p => (
                <Command.Item 
                  key={"proj-" + p.id}
                  onSelect={() => { onSelectProject(p.id); onClose(); }}
                  className="flex items-center gap-2 px-2 py-2.5 rounded-md cursor-pointer"
                >
                  <div className="w-3 h-3 rounded-full" style={{ backgroundColor: p.color }} /> {p.name}
                </Command.Item>
              ))}
            </Command.Group>

            <Command.Group heading={t('notes.palette.notes')} className="workspace-command-group--separated">
              {notes.map(n => (
                <Command.Item 
                  key={"note-" + n.id}
                  onSelect={() => { onSelectNote(n.id); onClose(); }}
                  className="flex flex-col items-start px-2 py-2 rounded-md cursor-pointer"
                >
                  <span className="font-semibold line-clamp-1">{n.title}</span>
                </Command.Item>
              ))}
            </Command.Group>

            <Command.Group heading={t('notes.palette.commands')} className="workspace-command-group--separated">
              <Command.Item 
                onSelect={() => { onOpenSettings(); onClose(); }}
                className="flex items-center gap-2 px-2 py-2.5 rounded-md cursor-pointer"
              >
                <Settings className="w-4 h-4" /> {t('notes.palette.settings')}
              </Command.Item>
            </Command.Group>

          </Command.List>
          <div className="workspace-command-footer" aria-hidden="true"><span><Kbd>↑</Kbd><Kbd>↓</Kbd> {t('notes.palette.browse')}</span><span><Kbd>{t('notes.palette.keyEnter')}</Kbd> {t('notes.palette.open')}</span><span><Kbd>{t('notes.palette.keyEscape')}</Kbd> {t('notes.palette.close')}</span></div>
        </Command>
    </Dialog>
  );
}

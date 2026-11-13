import { useEffect, useRef, useState, type Dispatch, type MutableRefObject, type SetStateAction } from 'react';
import type { NotesViewMode as ViewMode } from '../utils/viewMode';

import type { WorkspacePage } from '../types';

interface UseGlobalShortcutsOptions {
  handleCreateNote: () => void;
  workspacePage: WorkspacePage;
  /** Length of the full list: restarts the search for the element to focus. */
  projectNotesCount: number;
  /** True when a modal is open: shortcuts must not act behind it. */
  modalOpenRef: MutableRefObject<boolean>;
  /** Ctrl+F: the search of the whole space. */
  onSearch: () => void;
  /** Ctrl+1 to 4: the views in the order of the view switch. */
  onViewMode: (mode: ViewMode) => void;
  setIsCommandPaletteOpen: Dispatch<SetStateAction<boolean>>;
}

const VIEW_KEYS: Record<string, ViewMode> = { '1': 'plan', '2': 'cards', '3': 'list', '4': 'kanban' };

/** Global keyboard shortcuts (Ctrl+N, Ctrl+F, Ctrl+1 to 4), search focus and command palette opening (Ctrl+K). */
export function useGlobalShortcuts({
  handleCreateNote, workspacePage, projectNotesCount, modalOpenRef, onSearch, onViewMode, setIsCommandPaletteOpen,
}: UseGlobalShortcutsOptions) {
  const [searchFocusPending, setSearchFocusPending] = useState(false);
  const actionsRef = useRef({ onSearch, onViewMode });
  useEffect(() => { actionsRef.current = { onSearch, onViewMode }; });

  // Global keyboard shortcuts
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      // Shortcuts act on the active surface: no note creation, view change
      // or search focus behind an open modal.
      if (modalOpenRef.current || document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      // Caps Lock turns the key upper case: Ctrl+N stays Ctrl+N. Shift and Alt combinations are
      // left to the page and the system.
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
        const key = e.key.toLowerCase();
        if (key === 'n') {
          e.preventDefault();
          handleCreateNote();
        } else if (key === 'f') {
          e.preventDefault();
          // In a note, Ctrl+F does not leave writing (nor lose the selection): note
          // search is only offered from the other pages.
          if (workspacePage === 'editor') return;
          actionsRef.current.onSearch();
        } else if (VIEW_KEYS[key]) {
          e.preventDefault();
          actionsRef.current.onViewMode(VIEW_KEYS[key]);
        }
      }
    };
    window.addEventListener('keydown', handleGlobalKeyDown);
    return () => window.removeEventListener('keydown', handleGlobalKeyDown);
  }, [handleCreateNote, workspacePage, modalOpenRef]);

  useEffect(() => {
    if (!searchFocusPending || workspacePage !== 'notes') return;
    const frame = requestAnimationFrame(() => {
      const search = document.getElementById('search-input');
      if (search) search.focus();
      else setIsCommandPaletteOpen(true);
      setSearchFocusPending(false);
    });
    return () => cancelAnimationFrame(frame);
  }, [searchFocusPending, workspacePage, projectNotesCount, setIsCommandPaletteOpen]);

  // Command palette (Ctrl+K)
  useEffect(() => {
    const handleOpen = () => {
      // Ctrl+K must not stack the palette on top of another modal.
      if (modalOpenRef.current || document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      setIsCommandPaletteOpen(true);
    };
    document.addEventListener('open-command-palette', handleOpen);
    return () => document.removeEventListener('open-command-palette', handleOpen);
  }, [modalOpenRef, setIsCommandPaletteOpen]);

  return { setSearchFocusPending };
}

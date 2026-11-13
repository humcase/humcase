import type { ElementType } from 'react';
import { FileText, Gavel, Lightbulb, ListChecks, MagnifyingGlass, PencilSimpleLine, Question, WarningCircle } from '@phosphor-icons/react';
import type { NatureShortcut } from '../../utils/natureShortcuts';

/** Icon of each shortcut by nature (sidebar and view). */
export const SHORTCUT_ICONS: Record<NatureShortcut, ElementType> = {
  decisions: Gavel,
  tasks: ListChecks,
  documents: FileText,
  drawings: PencilSimpleLine,
  ideas: Lightbulb,
  questions: Question,
  research: MagnifyingGlass,
  issues: WarningCircle,
};

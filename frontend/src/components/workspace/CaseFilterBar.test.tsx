import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { CaseFilterBar, type CaseFilterBarProps } from './CaseFilterBar';

const props = (overrides: Partial<CaseFilterBarProps> = {}): CaseFilterBarProps => ({
  query: '', onQueryChange: vi.fn(), status: '', onStatusChange: vi.fn(),
  statusCounts: { all: 4, open: 2, in_progress: 1, resolved: 1 },
  nature: '', onNatureChange: vi.fn(), natureCounts: new Map([['todo', 3], ['idea', 1]]),
  subCases: false, subCaseCount: 6, onSubCasesChange: vi.fn(), today: false, onLeaveToday: vi.fn(), onClear: vi.fn(),
  ...overrides,
});

describe('CaseFilterBar', () => {
  it('counts the cases of each status, whatever the status chosen', () => {
    render(<CaseFilterBar {...props({ status: 'open' })} />);
    const counts = Object.fromEntries(['Toutes', 'À faire', 'En cours', 'Résolu'].map(name => [name, screen.getByRole('radio', { name }).querySelector('.ui-segmented__count')?.textContent]));
    expect(counts).toEqual({ Toutes: '4', 'À faire': '2', 'En cours': '1', Résolu: '1' });
  });

  it('offers only the natures present, and a filter in use can always be cleared', () => {
    const onClear = vi.fn();
    const { rerender } = render(<CaseFilterBar {...props({ onClear })} />);
    const onNatureChange = vi.fn();
    rerender(<CaseFilterBar {...props({ onClear, onNatureChange })} />);
    const natures = within(screen.getByRole('group', { name: 'Filtrer par nature' }));
    expect(natures.getAllByRole('button').map(button => button.textContent)).toEqual(['Toutes les natures', 'Idée1', 'Tâche3']);
    expect(screen.queryByRole('button', { name: 'Effacer les filtres' })).toBeNull();
    // One click chooses a nature; a click on the chosen one clears it.
    fireEvent.click(natures.getByRole('button', { name: /^Tâche/ }));
    expect(onNatureChange).toHaveBeenLastCalledWith('todo');
    rerender(<CaseFilterBar {...props({ onClear, onNatureChange, nature: 'todo' })} />);
    expect(screen.getByRole('button', { name: /^Tâche/ }).getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: /^Tâche/ }));
    expect(onNatureChange).toHaveBeenLastCalledWith('');
    rerender(<CaseFilterBar {...props({ onClear, nature: 'idea' })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Effacer les filtres' }));
    expect(onClear).toHaveBeenCalled();
  });

  it('shows "Today" as a filter that can be left, and the sub-cases choice only where it applies', () => {
    const onLeaveToday = vi.fn();
    const { rerender } = render(<CaseFilterBar {...props({ today: true, onLeaveToday, subCases: null })} />);
    expect(screen.queryByRole('checkbox', { name: /sous-cases/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /Aujourd’hui/ }));
    expect(onLeaveToday).toHaveBeenCalled();
    const onSubCasesChange = vi.fn();
    rerender(<CaseFilterBar {...props({ onSubCasesChange })} />);
    // A box whose state reads, with the number of sub-cases it adds.
    const box = screen.getByRole('checkbox', { name: /Inclure les sous-cases/ }) as HTMLInputElement;
    expect(box.checked).toBe(false);
    expect(box.closest('label')?.textContent).toContain('6');
    fireEvent.click(box);
    expect(onSubCasesChange).toHaveBeenCalledWith(true);
    rerender(<CaseFilterBar {...props({ subCases: true })} />);
    expect((screen.getByRole('checkbox', { name: /Inclure les sous-cases/ }) as HTMLInputElement).checked).toBe(true);
  });
});

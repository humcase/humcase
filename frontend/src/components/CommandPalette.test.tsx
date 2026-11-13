import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import CommandPalette from './CommandPalette';

// Review 0.9.69, no. 12: "No results" must not be a dead end.
describe('Command palette — search without results', () => {
  it('offers to clear the search and renders the results', () => {
    // cmdk measures the list; jsdom has neither ResizeObserver nor scrollIntoView.
    globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} } as unknown as typeof ResizeObserver;
    Element.prototype.scrollIntoView ??= vi.fn();
    render(<CommandPalette isOpen onClose={() => {}} projects={[]} notes={[]} onSelectProject={() => {}} onSelectNote={() => {}} onOpenSettings={() => {}} />);
    const input = screen.getByPlaceholderText('Rechercher une case, un projet, une commande…') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'zzzz' } });
    fireEvent.click(screen.getByRole('button', { name: 'Vider la recherche' }));
    expect(input.value).toBe('');
    expect(screen.getByText('Toutes les cases')).toBeTruthy();
  });
});

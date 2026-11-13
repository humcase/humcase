import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { Card } from './Card';
import { List, ListRow } from './List';
import { Notice, Toast } from './Toast';
import { EmptyState } from './EmptyState';

describe('Kit: cards, rows, feedback, empty states', () => {
  it('Card exposes its states through attributes, without a screen class', () => {
    const { container } = render(<Card as="article" interactive active>Contenu</Card>);
    const card = container.querySelector('article.ui-card')!;
    expect(card.hasAttribute('data-interactive')).toBe(true);
    expect(card.hasAttribute('data-active')).toBe(true);
  });

  it('ListRow: the action covers the row, inner buttons stay distinct', () => {
    const onOpen = vi.fn();
    const onTag = vi.fn();
    render(<List label="Notes"><ListRow title="Plan" onClick={onOpen} actionLabel="Ouvrir Plan" active><button type="button" onClick={onTag}>#tag</button></ListRow></List>);
    fireEvent.click(screen.getByRole('button', { name: 'Ouvrir Plan' }));
    expect(onOpen).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('button', { name: '#tag' }));
    expect(onTag).toHaveBeenCalledOnce();
    expect(onOpen).toHaveBeenCalledOnce();
    expect(screen.getByRole('button', { name: 'Ouvrir Plan' }).getAttribute('aria-current')).toBe('true');
  });

  it('Toast and Notice announce an error as an alert, the rest as a status', () => {
    render(<><Toast tone="danger">Échec</Toast><Notice>Enregistré</Notice></>);
    expect(screen.getByRole('alert').textContent).toContain('Échec');
    expect(screen.getByRole('status').textContent).toContain('Enregistré');
  });

  it('Toast: the remaining-time ring only appears with an action and a duration', () => {
    const { container, rerender } = render(<Toast duration={4000}>Note supprimée</Toast>);
    expect(container.querySelector('.ui-toast__timer')).toBeNull();
    rerender(<Toast duration={4000} action={<button type="button">Annuler</button>}>Note supprimée</Toast>);
    expect(container.querySelector('.ui-toast__timer')).toBeTruthy();
  });

  it('EmptyState: title at the requested level, fan only in md size', () => {
    const { container, rerender } = render(<EmptyState icon={<svg />} headingLevel={2} title="Vide" description="Rien ici" />);
    expect(screen.getByRole('heading', { level: 2, name: 'Vide' })).toBeTruthy();
    expect(container.querySelector('.ui-empty__keys')).toBeTruthy();
    rerender(<EmptyState size="sm" icon={<svg />} description="Rien ici" />);
    expect(container.querySelector('.ui-empty__keys')).toBeNull();
  });
});

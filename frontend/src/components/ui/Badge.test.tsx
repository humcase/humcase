import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PriorityBadge, StatusBadge } from './Badge';

describe('StatusBadge — vocabulary and distinction of statuses', () => {
  it("shows the single label of each status (no \"Terminé\")", () => {
    const { rerender } = render(<StatusBadge status="open" />);
    expect(screen.getByText('À faire')).toBeTruthy();
    rerender(<StatusBadge status="in_progress" />);
    expect(screen.getByText('En cours')).toBeTruthy();
    rerender(<StatusBadge status="resolved" />);
    expect(screen.getByText('Résolu')).toBeTruthy();
    expect(screen.queryByText('Terminé')).toBeNull();
    expect(screen.queryByText('Terminée')).toBeNull();
  });

  it('exposes the status through data-status (reinforcing color, not the only signal)', () => {
    const { container } = render(<StatusBadge status="resolved" />);
    expect(container.querySelector('.ui-status')?.getAttribute('data-status')).toBe('resolved');
  });

  it('distinguishes states by a different icon (readable in grayscale)', () => {
    const { container, rerender } = render(<StatusBadge status="open" />);
    const open = container.querySelector('.ui-status')?.innerHTML;
    rerender(<StatusBadge status="in_progress" />);
    const inProgress = container.querySelector('.ui-status')?.innerHTML;
    rerender(<StatusBadge status="resolved" />);
    const resolved = container.querySelector('.ui-status')?.innerHTML;
    expect(open).toBeTruthy();
    expect(inProgress).not.toBe(open);
    expect(resolved).not.toBe(open);
    expect(resolved).not.toBe(inProgress);
  });
});

describe('PriorityBadge — discreet priority on a resolved note', () => {
  it('only colors P0 (danger) and P1 (caution) on an active note', () => {
    const tone = (priority: string) => render(<PriorityBadge priority={priority} status="open" />).container.querySelector('.ui-priority')?.getAttribute('data-tone');
    expect(tone('P0')).toBe('danger');
    expect(tone('P1')).toBe('warning');
    expect(tone('P2')).toBe('neutral');
    expect(tone('P3')).toBe('neutral');
  });

  it('renders the priority neutral and recessed as soon as the note is resolved', () => {
    for (const priority of ['P0', 'P1', 'P2', 'P3']) {
      const badge = render(<PriorityBadge priority={priority} status="resolved" />).container.querySelector('.ui-priority');
      expect(badge?.getAttribute('data-tone')).toBe('neutral');
      expect(badge?.hasAttribute('data-resolved')).toBe(true);
    }
  });

  it('renders nothing without a priority', () => {
    expect(render(<PriorityBadge priority={null} status="open" />).container.innerHTML).toBe('');
  });
});

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DayTimeline } from './DayTimeline';

// Review 0.9.69, no. 4: a failed read of agent writes must not pass for a
// day without activity.
describe('Timeline "Your day" — failure to read agent writes', () => {
  it('announces the failure instead of "Nothing recorded"', () => {
    render(<DayTimeline notes={[]} revisions={[]} revisionsFailed onNote={() => {}} />);
    expect(screen.getByRole('alert').textContent).toContain('n’ont pas pu être lues');
    expect(screen.queryByText('Rien d’enregistré aujourd’hui pour l’instant.')).toBeNull();
  });

  it('keeps the real empty state when the read succeeded', () => {
    render(<DayTimeline notes={[]} revisions={[]} onNote={() => {}} />);
    expect(screen.getByText('Rien d’enregistré aujourd’hui pour l’instant.')).toBeTruthy();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

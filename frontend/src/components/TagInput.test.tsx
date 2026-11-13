import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import TagInput from './TagInput';

it('exposes suggestions as accessible buttons and adds the chosen tag only once', () => {
  const change = vi.fn();
  render(<TagInput tags={['source']} availableTags={['design']} onChange={change} />);
  fireEvent.change(screen.getByRole('textbox', { name: 'Ajouter un tag' }), { target: { value: 'des' } });
  const suggestion = screen.getByRole('button', { name: '#design' });
  suggestion.focus();
  expect(document.activeElement).toBe(suggestion);
  fireEvent.click(suggestion);
  expect(change).toHaveBeenCalledExactlyOnceWith(['source', 'design']);
});

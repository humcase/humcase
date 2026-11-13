import { describe, it, expect, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { Dialog, DialogHeader } from './Dialog';

/** Closing driven by the caller: `refuse` simulates a write in progress. */
function Harness({ refuse = false, closeOnOutside = false, onClose = () => {} }: { refuse?: boolean; closeOnOutside?: boolean; onClose?: () => void }) {
  const [open, setOpen] = useState(true);
  const close = () => { onClose(); if (!refuse) setOpen(false); };
  return <div className="workspace-app">
    <button type="button">Déclencheur</button>
    {open && <Dialog onClose={close} closeOnOutside={closeOnOutside}><DialogHeader title="Titre du test" onClose={close} /><input aria-label="Champ" /></Dialog>}
  </div>;
}

describe('Dialog du kit', () => {
  it('carries the name of its title and puts the focus in the dialog', () => {
    render(<Harness />);
    const dialog = screen.getByRole('dialog', { name: 'Titre du test' });
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('Escape goes through onClose, which may refuse the closing', () => {
    const onClose = vi.fn();
    render(<Harness refuse onClose={onClose} />);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
    expect(screen.getByRole('dialog', { name: 'Titre du test' })).toBeTruthy();
  });

  it('Escape closes when onClose accepts', () => {
    render(<Harness />);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('a click outside does not close, except closeOnOutside', async () => {
    // Radix installs the listener on the tick after opening and waits for the click that follows the press.
    const tick = () => act(() => new Promise(resolve => setTimeout(resolve, 20)));
    const onClose = vi.fn();
    const { unmount } = render(<Harness onClose={onClose} />);
    await tick();
    fireEvent.pointerDown(document.body); fireEvent.click(document.body);
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeTruthy();
    unmount();
    const onOutside = vi.fn();
    render(<Harness closeOnOutside onClose={onOutside} />);
    await tick();
    fireEvent.pointerDown(document.body); fireEvent.click(document.body);
    expect(onOutside).toHaveBeenCalledOnce();
  });
});

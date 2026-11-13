import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DueDateField } from './DueDateField';
import i18n from '../i18n';
import { parseDay } from '../utils/dueDates';

describe('Due date — calendar', () => {
  it('opening then closing does not change the saved date', () => {
    const change = vi.fn();
    render(<DueDateField value="2026-09-27" onChange={change} />);
    fireEvent.click(screen.getByRole('button', { name: 'Choisir l’échéance dans le calendrier' }));
    expect(screen.getByRole('dialog', { name: 'Calendrier d’échéance' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Fermer' }));
    expect(change).not.toHaveBeenCalled();
    expect(screen.getByLabelText('Échéance')).toHaveProperty('value', '27/09/2026');
  });
  it('passes a chosen local day, without UTC conversion', () => {
    const change = vi.fn();
    render(<DueDateField value="2026-09-27" onChange={change} />);
    fireEvent.click(screen.getByRole('button', { name: 'Choisir l’échéance dans le calendrier' }));
    fireEvent.click(screen.getByRole('button', { name: /28 septembre 2026/ }));
    expect(change).toHaveBeenCalledExactlyOnceWith('2026-09-28');
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('Due date — typed field (not the native one, drawn in the language of Windows)', () => {
  const onLanguages = (languages: string[]) => vi.spyOn(navigator, 'languages', 'get').mockReturnValue(languages);

  it('in English on a French Windows: day/month/year and an English hint, never jj/mm/aaaa', async () => {
    const languages = onLanguages(['fr-FR', 'fr', 'en-US', 'en']);
    await act(() => i18n.changeLanguage('en'));
    try {
      const { unmount } = render(<DueDateField value="" onChange={() => {}} />);
      expect(screen.getByLabelText('Due date')).toHaveProperty('placeholder', 'dd/mm/yyyy');
      unmount();
      render(<DueDateField value="2026-10-17" onChange={() => {}} />);
      expect(screen.getByLabelText('Due date')).toHaveProperty('value', '17/10/2026');
    } finally {
      await act(() => i18n.changeLanguage('fr'));
      languages.mockRestore();
    }
  });

  it('in English on a US computer: month first, like every other date', async () => {
    const languages = onLanguages(['en-US', 'en']);
    await act(() => i18n.changeLanguage('en'));
    try {
      render(<DueDateField value="" onChange={() => {}} />);
      expect(screen.getByLabelText('Due date')).toHaveProperty('placeholder', 'mm/dd/yyyy');
      expect(parseDay('10/17/2026')).toBe('2026-10-17');
    } finally {
      await act(() => i18n.changeLanguage('fr'));
      languages.mockRestore();
    }
  });

  it('in French: jj/mm/aaaa', () => {
    render(<DueDateField value="" onChange={() => {}} />);
    expect(screen.getByLabelText('Échéance')).toHaveProperty('placeholder', 'jj/mm/aaaa');
  });

  it('a typed day is saved on Enter or on leaving the field', () => {
    const change = vi.fn();
    render(<DueDateField value="2026-09-27" onChange={change} />);
    const field = screen.getByLabelText('Échéance');
    fireEvent.change(field, { target: { value: '3/10/26' } });
    expect(change).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(change).toHaveBeenCalledExactlyOnceWith('2026-10-03');
    fireEvent.change(field, { target: { value: '05102026' } });
    fireEvent.blur(field);
    expect(change).toHaveBeenLastCalledWith('2026-10-05');
  });

  it('a text that is not a day keeps the saved date; an emptied field clears it; Escape cancels', () => {
    const change = vi.fn();
    render(<DueDateField value="2026-09-27" onChange={change} />);
    const field = screen.getByLabelText('Échéance');
    fireEvent.change(field, { target: { value: '31/02/2026' } });
    fireEvent.blur(field);
    expect(change).not.toHaveBeenCalled();
    expect(field).toHaveProperty('value', '27/09/2026');
    fireEvent.change(field, { target: { value: '01/01/2027' } });
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(field).toHaveProperty('value', '27/09/2026');
    fireEvent.blur(field);
    expect(change).not.toHaveBeenCalled();
    fireEvent.change(field, { target: { value: '' } });
    fireEvent.blur(field);
    expect(change).toHaveBeenCalledExactlyOnceWith('');
  });

  it('reads the day in the order of the dates shown, or as YYYY-MM-DD', () => {
    expect(parseDay('31/12/2026')).toBe('2026-12-31');
    expect(parseDay('31.12.2026')).toBe('2026-12-31');
    expect(parseDay('2026-12-31')).toBe('2026-12-31');
    expect(parseDay('29/02/2028')).toBe('2028-02-29');
    expect(parseDay('29/02/2027')).toBeNull();
    expect(parseDay('12/31/2026')).toBeNull();
    expect(parseDay('demain')).toBeNull();
    expect(parseDay('31/12')).toBeNull();
  });
});

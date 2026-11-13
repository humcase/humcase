import { useState } from 'react';

const STORAGE_KEY = 'humcase-include-sub-cases';

function readChoice(): boolean {
  try { return localStorage.getItem(STORAGE_KEY) === 'true'; } catch { return false; }
}

// "Include sub-cases" of the cases page: one choice of the person, unticked at first, kept across
// launches. A status, a nature or "Clear filters" never changes it (founder decision, 05/10).
export function useSubCasesChoice() {
  const [shown, setShown] = useState(readChoice);
  const choose = (include: boolean) => {
    setShown(include);
    try { localStorage.setItem(STORAGE_KEY, String(include)); } catch { /* kept for this session only */ }
  };
  return { shown, choose };
}

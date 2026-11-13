import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../../services/tauriApi';

/**
 * Whether an agent connected through MCP may edit cases (`update_note`). `null` = not yet read:
 * the toggle stays inactive until the Rust truth has been read (never an optimistic value). It is
 * read when the section opens, not when Settings open, and a late answer is ignored.
 */
export function useAgentPermission(sectionOpen: boolean) {
  const { t } = useTranslation();
  const [allowNoteEdit, setAllowNoteEdit] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!sectionOpen) return;
    let disposed = false;
    api
      .getMcpPermissions()
      .then(permissions => {
        if (!disposed) setAllowNoteEdit(permissions.allow_note_edit);
      })
      .catch(e => {
        if (!disposed) setError(t('settings.agent.unavailable', { error: String(e) }));
      });
    return () => {
      disposed = true;
    };
  }, [sectionOpen, t]);

  /** Toggles the permission; the local state only changes after Rust confirms. */
  const change = async (enabled: boolean) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.setMcpAllowNoteEdit(enabled);
      setAllowNoteEdit(enabled);
    } catch (e) {
      setError(t('settings.agent.saveFailed', { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return { allowNoteEdit, busy, error, change, clearError: () => setError(null) };
}

export type AgentPermission = ReturnType<typeof useAgentPermission>;

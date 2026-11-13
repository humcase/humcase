import { useTranslation } from 'react-i18next';
import { Card } from '../ui/Card';
import type { AgentPermission } from './useAgentPermission';
import { AgentConnect } from './AgentConnect';

/** Agents: how to connect one, whether one is connected, and what it may edit in the cases. */
export function AgentSection({ permission }: { permission: AgentPermission }) {
  const { t } = useTranslation();
  return (
    <div className="settings-panel">
      <AgentConnect />
      <div className="settings-section-heading settings-agent-permission-heading"><h3>{t('settings.agent.connect.permissionHeading')}</h3></div>
      <Card padding="none" className="settings-card">
        <label htmlFor="mcp-allow-note-edit" className="settings-label flex items-center gap-2">
          <input
            id="mcp-allow-note-edit"
            type="checkbox"
            checked={permission.allowNoteEdit === true}
            disabled={permission.allowNoteEdit === null || permission.busy}
            onChange={event => { void permission.change(event.target.checked); }}
            className="accent-primary"
          />
          {t('settings.agent.allowEdit')}
        </label>
        <p className="text-xs text-muted-foreground">{t('settings.agent.permissionText')}</p>
        {permission.allowNoteEdit === null && !permission.error && (
          <span role="status" className="text-[11px] text-muted-foreground">{t('settings.agent.loading')}</span>
        )}
        {permission.error && (
          <p role="alert" className="settings-alert">
            {permission.error}
          </p>
        )}
      </Card>
    </div>
  );
}

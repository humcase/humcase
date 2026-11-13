import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Copy } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SegmentedControl } from '../ui/SegmentedControl';
import { api } from '../../services/tauriApi';
import { AGENT_TOOLS, agentSetup, type AgentTool } from '../../utils/agentSetup';
import { formatRelative } from '../../utils/datetime';
import { useAgentsSeen } from '../../hooks/useAgentsSeen';

const TOOL_NAMES: Record<AgentTool, string> = { claude: 'Claude', codex: 'Codex', cursor: 'Cursor', opencode: 'opencode' };

/**
 * Settings → Agents, first part: whether an agent has connected, and how to connect one, with
 * this Humcase's path already written into each client's command or configuration.
 */
export function AgentConnect() {
  const { t } = useTranslation();
  const { agents, reload } = useAgentsSeen(undefined, 4000);
  const [tool, setTool] = useState<AgentTool>('claude');
  const [exePath, setExePath] = useState<string | null>(null);
  const [pathError, setPathError] = useState<string | null>(null);
  const [copy, setCopy] = useState<'idle' | 'copied' | 'failed'>('idle');

  useEffect(() => {
    let active = true;
    Promise.resolve(api.humcaseExecutablePath?.())
      .then(path => { if (active) setExePath(path ?? null); })
      .catch(cause => { if (active) setPathError(String(cause)); });
    return () => { active = false; };
  }, []);

  const setup = agentSetup(tool, exePath ?? 'Humcase.exe');
  const [latest, ...others] = agents ?? [];
  const copyCode = () => {
    navigator.clipboard.writeText(setup.code).then(() => setCopy('copied'), () => setCopy('failed'));
  };

  return (
    <div className="settings-agent-connect">
      <div className="settings-section-heading"><h3>{t('settings.agent.connect.heading')}</h3><p>{t('settings.agent.connect.description')}</p></div>

      <Card padding="none" className="settings-card settings-agent-status" data-connected={latest ? '' : undefined}>
        <span className="settings-agent-status__dot" aria-hidden="true" />
        <div role="status" className="settings-agent-status__text">
          <strong>{latest ? t('settings.agent.connect.seen', { name: latest.name }) : t('settings.agent.connect.none')}</strong>
          <span>{latest
            ? t('settings.agent.connect.seenText', { when: formatRelative(new Date(latest.at_unix_ms).toISOString()) })
            : t('settings.agent.connect.noneText')}</span>
          {others.length > 0 && <span>{t('settings.agent.connect.others', { names: others.map(agent => agent.name).join(', ') })}</span>}
        </div>
        <Button type="button" variant="secondary" size="sm" onClick={reload}>{t('settings.agent.connect.check')}</Button>
      </Card>

      <Card padding="none" className="settings-card settings-agent-steps">
        <div className="settings-agent-step">
          <h4>{t('settings.agent.connect.toolLabel')}</h4>
          <SegmentedControl variant="tabs" label={t('settings.agent.connect.toolLabel')} value={tool}
            onValueChange={value => { setTool(value); setCopy('idle'); }}
            items={AGENT_TOOLS.map(id => ({ value: id, label: id === 'claude' ? t('settings.agent.connect.toolClaude') : TOOL_NAMES[id] }))} />
        </div>
        <div className="settings-agent-step">
          <h4>{setup.file ? t('settings.agent.connect.whereFile', { file: setup.file }) : t('settings.agent.connect.whereTerminal')}</h4>
          <div className="settings-agent-code">
            <pre tabIndex={0}>{setup.code}</pre>
            <Button type="button" size="sm" onClick={copyCode} disabled={!exePath}>
              {copy === 'copied' ? <Check size={15} aria-hidden="true" /> : <Copy size={15} aria-hidden="true" />}
              {copy === 'copied' ? t('settings.agent.connect.copied') : t('settings.agent.connect.copy')}
            </Button>
          </div>
          {copy === 'failed' && <p role="alert" className="settings-alert">{t('settings.agent.connect.copyFailed')}</p>}
          {pathError
            ? <p role="alert" className="settings-alert">{t('settings.agent.connect.pathUnknown', { error: pathError })}</p>
            : <p className="settings-agent-step__note">{t('settings.agent.connect.pathNote')}</p>}
        </div>
        <div className="settings-agent-step">
          <h4>{t('settings.agent.connect.restart', { tool: TOOL_NAMES[tool] })}</h4>
          <p className="settings-agent-step__prompt">{t('settings.agent.connect.prompt')}</p>
        </div>
      </Card>
    </div>
  );
}

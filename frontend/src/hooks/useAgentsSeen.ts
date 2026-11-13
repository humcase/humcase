import { useCallback, useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import type { AgentSeen } from '../types';

/**
 * Agents that opened a session with this profile (`agents-seen.jsonl`), most recent first, read
 * again when `refreshKey` changes (the cases changed) and when `reload` is called. `null` while the
 * first read runs; a failed read counts as no agent seen yet, never as an error to show. With
 * `pollMs`, read again at that pace (an agent that only reads changes no case).
 */
export function useAgentsSeen(refreshKey?: unknown, pollMs?: number): { agents: AgentSeen[] | null; reload: () => void } {
  const [agents, setAgents] = useState<AgentSeen[] | null>(null);
  const [reloads, setReloads] = useState(0);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.listAgentsSeen?.())
      .then(result => { if (active) setAgents(result ?? []); })
      .catch(() => { if (active) setAgents([]); });
    return () => { active = false; };
  }, [refreshKey, reloads]);
  useEffect(() => {
    if (!pollMs) return;
    const timer = window.setInterval(() => setReloads(count => count + 1), pollMs);
    return () => window.clearInterval(timer);
  }, [pollMs]);
  const reload = useCallback(() => setReloads(count => count + 1), []);
  return { agents, reload };
}

import { useEffect, useState } from 'react';
import { api } from '../../services/tauriApi';
import type { AiUsageRecord } from '../../types';
import { periodStart, type UsagePeriod } from '../../utils/aiUsage';

interface Reading {
  period: UsagePeriod;
  now: Date;
  records: AiUsageRecord[] | null;
  error: string | null;
}

/**
 * The records of the chosen period, read when the Consumption section opens and on every period
 * change. A reading of another period is not shown: the section waits for the current one, and a
 * slower answer of an earlier period never replaces it.
 */
export function useAiUsage(period: UsagePeriod) {
  const [reading, setReading] = useState<Reading | null>(null);

  useEffect(() => {
    let current = true;
    const now = new Date();
    api.readAiUsage(periodStart(period, now).getTime()).then(
      records => { if (current) setReading({ period, now, records, error: null }); },
      e => { if (current) setReading({ period, now, records: null, error: String(e) }); },
    );
    return () => { current = false; };
  }, [period]);

  const ready = reading?.period === period ? reading : null;
  return { records: ready?.records ?? null, error: ready?.error ?? null, now: ready?.now ?? null };
}

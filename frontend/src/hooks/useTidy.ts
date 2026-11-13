import { useEffect, useState } from 'react';
import { api } from '../services/tauriApi';
import type { TidyItem } from '../types';
import { onHomeSettingsChanged } from '../utils/homeSettings';

/**
 * How often "To tidy" looks again (new pairs of decisions, cases resolved long enough ago). A
 * decision written elsewhere (an agent, another window) is read by meaning within seconds; the
 * pairs only meaning finds show at the next look.
 */
const LOOK_AGAIN_MS = 2 * 60_000;

const shown = (item: TidyItem, visibleIds: ReadonlySet<string>) =>
  visibleIds.has(item.case.id) && (!item.other || visibleIds.has(item.other.id));

/**
 * The proposals of "To tidy" among the cases shown (`visibleIds`): read again when the cases
 * change or when asked, and looked for again on mount then every two minutes (Jev reads the new
 * pairs then, without holding the database). Nothing is changed from here.
 */
export function useTidy(visibleIds: ReadonlySet<string>, notes: unknown): { items: TidyItem[]; weeks: number; reload: () => void } {
  const [items, setItems] = useState<TidyItem[]>([]);
  const [weeks, setWeeks] = useState(4);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let active = true;
    const look = (refresh: boolean) => Promise.resolve(api.tidyList?.(null, refresh))
      .then(next => { if (active) setItems(next ?? []); })
      .catch(() => { if (active) setItems([]); });
    void look(true);
    const timer = setInterval(() => { void look(true); }, LOOK_AGAIN_MS);
    // A setting changed (the archive delay, a search turned on or off): looked for again now.
    const stop = onHomeSettingsChanged(() => { void look(true); setTick(value => value + 1); });
    return () => { active = false; clearInterval(timer); stop(); };
  }, []);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.tidyList?.(null, false))
      .then(next => { if (active && next) setItems(next); })
      .catch(() => undefined);
    Promise.resolve(api.tidySettings?.())
      .then(settings => { if (active && settings) setWeeks(settings.archive_weeks); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [notes, tick]);
  return { items: items.filter(item => shown(item, visibleIds)), weeks, reload: () => setTick(value => value + 1) };
}

/** Ids of the decisions marked as replaced, read again when the cases change (search order). */
export function useSupersededIds(notes: unknown): ReadonlySet<string> {
  const [ids, setIds] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    let active = true;
    Promise.resolve(api.supersededIds?.())
      .then(next => { if (active) setIds(new Set(next ?? [])); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [notes]);
  return ids;
}

import { useEffect, useState } from 'react';

/** Milliseconds until the next midnight, local time. */
export function untilMidnight(now: Date): number {
  const next = new Date(now);
  next.setHours(24, 0, 0, 0);
  return next.getTime() - now.getTime();
}

/**
 * The current moment, renewed at midnight: "Today" and Home's follow-up are derived once per day
 * and per change of the cases, not on every render.
 */
export function useCurrentDay(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setTimeout(() => setNow(new Date()), untilMidnight(now) + 1000);
    return () => clearTimeout(timer);
  }, [now]);
  return now;
}

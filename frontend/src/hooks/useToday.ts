import { useEffect, useState } from 'react';

/**
 * The current moment, read once and read again at the next midnight, so a render stays pure
 * (the same date for every render of the day) and a window left open overnight moves on.
 */
export function useToday(): Date {
  const [today, setToday] = useState(() => new Date());
  useEffect(() => {
    const next = new Date(today);
    next.setHours(24, 0, 0, 0);
    const timer = setTimeout(() => setToday(new Date()), next.getTime() - today.getTime());
    return () => clearTimeout(timer);
  }, [today]);
  return today;
}

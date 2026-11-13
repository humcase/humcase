// A file of public/ with a fingerprint of its content in the address: a new version gets a new address,
// so these files can be cached for good and a visitor never runs a script older than the page.
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const cache = new Map<string, string>();
export function versioned(path: string): string {
  if (!cache.has(path)) {
    const hash = createHash('sha256').update(readFileSync(join(process.cwd(), 'public', path))).digest('hex').slice(0, 10);
    cache.set(path, `${path}?v=${hash}`);
  }
  return cache.get(path)!;
}

import { describe, expect, it } from 'vitest';
import viteConfig from '../../vite.config.ts?raw';
import showcaseEntry from './kit-showcase.tsx?raw';

describe('Kit showcase — development only', () => {
  it('is not a production build entry', () => {
    const input = viteConfig.match(/input:\s*\{([^}]*)\}/)?.[1] ?? '';
    expect(input).toMatch(/index\.html/);
    expect(input).not.toMatch(/kit/);
  });

  it('refuses to start outside development', () => {
    expect(showcaseEntry).toMatch(/if \(!import\.meta\.env\.DEV\) throw/);
  });
});

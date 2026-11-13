import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import i18n from '../i18n';

// Existing tests check French texts: the interface is forced to French there.
void i18n.changeLanguage('fr');

// Unmounts the React tree between each test (Vitest globals are not enabled).
afterEach(() => {
  cleanup();
});

// jsdom does not implement a few APIs used by the components.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

if (!('ResizeObserver' in globalThis)) {
  globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
}

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

// `useIsMobile` (shadcn Sidebar) listens to a media query.
if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false, media: query, onchange: null,
    addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

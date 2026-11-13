import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`,
  invoke: vi.fn(),
}));

import { firstLocalNoteImage } from './noteImage';
import { setImagesFolder } from './imageRefs';

describe('Local image preview', () => {
  it('displays the portable reference of a note image (0.9.81)', () => {
    setImagesFolder('/data/com.humcase.desktop/images');
    try {
      expect(firstLocalNoteImage('Texte\n\n![Capture](humcase-image://abc.webp)')).toEqual({
        src: `http://asset.localhost/${encodeURIComponent('/data/com.humcase.desktop/images/abc.webp')}`,
        alt: 'Capture',
      });
    } finally {
      setImagesFolder(null);
    }
  });

  it('finds an inline image and a reference defined further down', () => {
    expect(firstLocalNoteImage('![Plan](http://asset.localhost/C%3A/notes/plan.png)')).toEqual({ src: 'http://asset.localhost/C%3A/notes/plan.png', alt: 'Plan' });
    expect(firstLocalNoteImage('![Capture][figure]\n\n[figure]: asset://localhost/images/capture.png')).toEqual({ src: 'asset://localhost/images/capture.png', alt: 'Capture' });
  });
  it('ignores code, HTML and remote images, including deceptive hosts', () => {
    for (const content of [
      '```md\n![faux](http://asset.localhost/image.png)\n```',
      '<img src="http://asset.localhost/image.png">',
      '![externe](https://example.test/image.png)',
      '![faux](http://asset.localhost.example.test/image.png)',
      '![faux](http://asset.localhost@external.test/image.png)',
      '![donnée](data:image/png;base64,a)',
    ]) expect(firstLocalNoteImage(content)).toBeUndefined();
  });
  it('keeps the first Markdown definition and finds the first local image', () => {
    expect(firstLocalNoteImage('![Distant](https://example.test/image.png)\n![Local](http://asset.localhost/local.png)')?.alt).toBe('Local');
    expect(firstLocalNoteImage('![x][a]\n\n[a]: https://example.test/a.png\n[a]: http://asset.localhost/b.png')).toBeUndefined();
  });
});

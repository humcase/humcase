import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`,
  invoke: vi.fn(),
}));

import { displayImage, setImagesFolder, referenceFromDisplay, transformerUrlMarkdown } from './imageRefs';

const FOLDER = 'C:\\Users\\x\\.humcase\\images';

describe('imageRefs — portable image references (0.9.81)', () => {
  afterEach(() => setImagesFolder(null));

  it('displays a portable reference through the local URL of the images folder', () => {
    setImagesFolder(FOLDER);
    expect(displayImage('humcase-image://abc.webp')).toBe(`http://asset.localhost/${encodeURIComponent(`${FOLDER}\\abc.webp`)}`);
    expect(displayImage('https://exemple.fr/a.png')).toBe('https://exemple.fr/a.png');
    expect(displayImage('humcase-image://../secret.png')).toBe('humcase-image://../secret.png');
  });

  it('converts back an old local URL of an internal image, and only that one', () => {
    const internal = `http://asset.localhost/${encodeURIComponent(`${FOLDER}\\abc.png`)}`;
    const oldFolder = 'asset://localhost/C:/Users/x/AppData/Roaming/com.example.devnotes/images/def.jpg';
    const appData = `http://asset.localhost/${encodeURIComponent('C:\\Users\\x\\AppData\\Roaming\\com.humcase.desktop\\images\\ghi.png')}`;
    const elsewhere = `http://asset.localhost/${encodeURIComponent('C:\\Photos\\vacances.png')}`;
    expect(referenceFromDisplay(internal)).toBe('humcase-image://abc.png');
    expect(referenceFromDisplay(oldFolder)).toBe('humcase-image://def.jpg');
    expect(referenceFromDisplay(appData)).toBe('humcase-image://ghi.png');
    expect(referenceFromDisplay(elsewhere)).toBe(elsewhere);
    expect(referenceFromDisplay('humcase-image://abc.png')).toBe('humcase-image://abc.png');
  });

  it('ReactMarkdown keeps portable references and always filters dangerous protocols', () => {
    setImagesFolder(FOLDER);
    expect(transformerUrlMarkdown('humcase-image://abc.png')).toContain('asset.localhost');
    expect(transformerUrlMarkdown('javascript:alert(1)')).toBe('');
  });
});

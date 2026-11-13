// Builds the lighter copies of the site's images. Run after replacing a screenshot in
// public/shots or a maple frame in public/maple/px: npm run images
// - screenshots: AVIF and WebP at full width, and a 960 px set (AVIF, WebP, JPEG) for
//   one-column phones and two-column desktops at 1x; the JPEG stays the fallback
// - maple frames: an AVIF copy of each WebP (world.js falls back to the WebP)
import sharp from 'sharp';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const pub = fileURLToPath(new URL('../public/', import.meta.url));

const shots = join(pub, 'shots');
for (const f of readdirSync(shots).filter(f => /^app-[a-z]+-[a-z]+\.jpg$/.test(f))) {
  const base = join(shots, f.replace(/\.jpg$/, ''));
  for (const w of [1600, 960]) {
    const suffix = w === 1600 ? '' : '-' + w;
    const img = () => sharp(join(shots, f)).resize({ width: w, withoutEnlargement: true });
    // 4:4:4 keeps the small interface text crisp; the default 4:2:0 blurs coloured labels
    await img().avif({ quality: 64, effort: 7, chromaSubsampling: '4:4:4' }).toFile(`${base}${suffix}.avif`);
    await img().webp({ quality: 86, effort: 6, smartSubsample: true }).toFile(`${base}${suffix}.webp`);
    if (w !== 1600) await img().jpeg({ quality: 84, mozjpeg: true }).toFile(`${base}${suffix}.jpg`);
  }
  console.log('shot', f);
}

// the frames are dithered into pixels on the page: the AVIF copies change a few dither cells, nothing visible
const px = join(pub, 'maple', 'px');
for (const f of readdirSync(px).filter(f => f.endsWith('.webp'))) {
  await sharp(join(px, f)).avif({ quality: 70, effort: 7, chromaSubsampling: '4:4:4' }).toFile(join(px, f.replace(/\.webp$/, '.avif')));
}
console.log('maple frames');

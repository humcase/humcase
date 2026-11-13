import { convertFileSrc } from '@tauri-apps/api/core';
import { defaultUrlTransform } from 'react-markdown';
import { api } from '../services/tauriApi';

/**
 * Note images (0.9.81): the Markdown keeps a portable reference `humcase-image://<name>`,
 * valid on any machine; only display translates it to a local URL (`asset.localhost`).
 * Old local URLs of internal images are recognized and converted back to a portable reference
 * on save. Same rule as `src-tauri/src/images.rs`.
 */
export const SCHEMA_IMAGE = 'humcase-image://';
/**
 * Humcase's data folder `.humcase`, the AppData folder `com.humcase.desktop` of versions before
 * 0.10.0, and the `com.<owner>.devnotes` folder of versions up to 0.9.72.
 */
const IMAGES_SEGMENT = /\/(\.humcase|com\.humcase\.desktop|com\.[^/]+\.devnotes)\/images\//;
const VALID_NAME = /^[A-Za-z0-9._-]{1,128}$/;

let imagesFolder: string | null = null;

/** Reads the images folder once; without it, a portable reference cannot be displayed. */
export async function loadImagesFolder(): Promise<void> {
  try {
    imagesFolder = await api.getImagesDir();
  } catch {
    // Outside Tauri (tests, web): references stay as they are.
  }
}

/** For tests. */
export function setImagesFolder(folder: string | null) {
  imagesFolder = folder;
}

function validName(fileName: string): boolean {
  return VALID_NAME.test(fileName) && fileName !== '.' && fileName !== '..';
}

/** Displayable URL of an image source; any other source is returned as is. */
export function displayImage(src: string): string {
  if (!src.startsWith(SCHEMA_IMAGE) || !imagesFolder) return src;
  const fileName = src.slice(SCHEMA_IMAGE.length);
  if (!validName(fileName)) return src;
  const separator = imagesFolder.includes('\\') ? '\\' : '/';
  return convertFileSrc(`${imagesFolder}${separator}${fileName}`);
}

/** Portable reference of a local URL pointing into Humcase's images folder, otherwise `src`. */
export function referenceFromDisplay(src: string): string {
  let url: URL;
  try { url = new URL(src); } catch { return src; }
  const locale = (url.protocol === 'http:' && url.hostname === 'asset.localhost')
    || (url.protocol === 'asset:' && url.hostname === 'localhost');
  if (!locale) return src;
  let path: string;
  try { path = decodeURIComponent(url.pathname).replace(/\\/g, '/'); } catch { return src; }
  const lowercase = path.toLowerCase();
  const match = IMAGES_SEGMENT.exec(lowercase);
  if (!match) return src;
  const fileName = path.slice(match.index + match[0].length);
  return validName(fileName) ? `${SCHEMA_IMAGE}${fileName}` : src;
}

/** For `ReactMarkdown`: displays portable references, filters the rest as by default. */
export function transformerUrlMarkdown(url: string): string {
  return url.startsWith(SCHEMA_IMAGE) ? displayImage(url) : defaultUrlTransform(url);
}

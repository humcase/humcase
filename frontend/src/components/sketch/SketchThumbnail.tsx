import { useEffect, useMemo, useState } from 'react';
import { PencilSimpleLine } from '@phosphor-icons/react';
import { sketchScene } from '../../utils/caseContent';
import { loadExcalidraw } from './loadExcalidraw';

/** Pictures already drawn, by scene: a board shows each one once. */
const pictures = new Map<string, string>();
const MAX_PICTURES = 300;

function remember(key: string, url: string) {
  pictures.set(key, url);
  if (pictures.size > MAX_PICTURES) {
    const [oldest, oldUrl] = pictures.entries().next().value as [string, string];
    pictures.delete(oldest);
    if (oldUrl) URL.revokeObjectURL(oldUrl);
  }
}

/**
 * Small picture of a sketch, drawn by Excalidraw from the scene. It is shown as an image (a
 * blob, never inserted markup), so nothing in a drawing can run in the app. Text keeps a system
 * font: the drawing's fonts are not embedded in the small picture. The dark theme inverts the
 * picture as Excalidraw inverts its canvas (`sketch.css`).
 */
export default function SketchThumbnail({ content, label }: { content: string; label: string }) {
  const key = content;
  const [shown, setShown] = useState<{ key: string; url: string } | null>(() => {
    const url = pictures.get(key);
    return url === undefined ? null : { key, url };
  });
  // An empty or unreadable sketch has no picture to draw.
  const drawing = useMemo(() => {
    const scene = sketchScene(content);
    const elements = scene?.elements.filter(element => !element.isDeleted) ?? [];
    return scene && elements.length ? { scene, elements } : null;
  }, [content]);
  const url = !drawing ? '' : shown?.key === key ? shown.url : pictures.get(key);

  useEffect(() => {
    if (!drawing || pictures.has(key)) return;
    let alive = true;
    const { scene, elements } = drawing;
    loadExcalidraw()
      .then(module => module.exportToSvg({
        elements: elements as never,
        appState: { ...scene.appState, exportBackground: false, exportWithDarkMode: false } as never,
        files: scene.files as never,
        exportPadding: 8,
        skipInliningFonts: true,
      }))
      .then(svg => {
        const blob = new Blob([new XMLSerializer().serializeToString(svg)], { type: 'image/svg+xml' });
        const made = URL.createObjectURL(blob);
        remember(key, made);
        if (alive) setShown({ key, url: made });
      })
      .catch(() => {
        remember(key, '');
        if (alive) setShown({ key, url: '' });
      });
    return () => { alive = false; };
  }, [drawing, key]);

  if (!url) {
    return (
      <div className={`case-sketch-thumb is-empty${url === undefined ? ' is-loading' : ''}`} aria-hidden="true">
        <PencilSimpleLine size={22} />
      </div>
    );
  }
  return <div className="case-sketch-thumb"><img src={url} alt={label} draggable={false} /></div>;
}

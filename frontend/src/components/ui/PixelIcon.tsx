import type { ReactElement } from 'react';

// 12 × 12 pixel icons of the "game object" universe: one pixel = one cell, crisp rendering.
const GLYPHS = {
  // Note page with a folded corner.
  note: [
    '.#######....',
    '.#.....##...',
    '.#.....#.#..',
    '.#.....####.',
    '.#........#.',
    '.#.#####..#.',
    '.#........#.',
    '.#.######.#.',
    '.#........#.',
    '.#.####...#.',
    '.#........#.',
    '.##########.',
  ],
  // Agent: small robot with antennas.
  agent: [
    '...#....#...',
    '....#..#....',
    '..########..',
    '.#........#.',
    '.#.##..##.#.',
    '.#.##..##.#.',
    '.#........#.',
    '.#..####..#.',
    '..########..',
    '...#....#...',
    '..##....##..',
    '............',
  ],
} as const;

export type PixelGlyph = keyof typeof GLYPHS;

export function PixelIcon({ glyph, size = 16, className }: { glyph: PixelGlyph; size?: number; className?: string }) {
  const cells: ReactElement[] = [];
  GLYPHS[glyph].forEach((row, y) => {
    for (let x = 0; x < row.length; x += 1) {
      if (row[x] === '#') cells.push(<rect key={`${x}-${y}`} x={x} y={y} width={1} height={1} />);
    }
  });
  return <svg viewBox="0 0 12 12" width={size} height={size} fill="currentColor" shapeRendering="crispEdges" aria-hidden="true" className={className}>{cells}</svg>;
}

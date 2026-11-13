import { useId } from 'react';

// Caption "h" vectorized from DM Mono 500 (no dependency on the loaded font: icons, exports).
const LEGEND_H = 'M26.1 40V21.28H28.85V29.03Q29.37 28.07 30.52 27.43Q31.66 26.79 33.09 26.79Q35.48 26.79 36.73 28.23Q37.98 29.68 37.98 32.38V40H35.22V32.77Q35.22 31.16 34.44 30.28Q33.66 29.39 32.29 29.39Q30.86 29.39 29.85 30.35Q28.85 31.32 28.85 33.14V40Z';

/**
 * Humcase logo: an orange key screen-printed "h", raised on its shadow, with its indicator light,
 * framed to fill its square like other apps' icons.
 * The same drawing as the app icon and the website (`src-tauri/icons/source.svg`,
 * `site/public/favicon.svg`); its colors are the brand's, so they do not follow the theme.
 */
export function HumcaseKey({ size = 32, className }: { size?: number; className?: string }) {
  const id = `humcase-key-${useId().replace(/:/g, '')}`;
  return (
    <svg className={className ? `humcase-key ${className}` : 'humcase-key'} width={size} height={size} viewBox="4.6 1.6 54.8 54.8" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id={`${id}-f`} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#f07a3b" /><stop offset="1" stopColor="#dc5d24" /></linearGradient>
        <linearGradient id={`${id}-d`} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#cf541d" /><stop offset=".55" stopColor="#dc5d24" /><stop offset="1" stopColor="#f08348" /></linearGradient>
        <linearGradient id={`${id}-h`} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#ffe3cc" stopOpacity=".75" /><stop offset=".3" stopColor="#ffe3cc" stopOpacity="0" /></linearGradient>
        <clipPath id={`${id}-c`}><rect x="6" y="3" width="52" height="50" rx="13" /></clipPath>
        <filter id={`${id}-s`} x="-.3" y="-.3" width="1.6" height="1.6"><feGaussianBlur stdDeviation="1.2" /></filter>
        <filter id={`${id}-g`} x="-1" y="-1" width="3" height="3"><feGaussianBlur stdDeviation="1.6" /></filter>
      </defs>
      <rect x="8" y="6" width="48" height="49" rx="13" fill="#000" opacity=".4" filter={`url(#${id}-s)`} />
      <rect x="6" y="3" width="52" height="50" rx="13" fill={`url(#${id}-f)`} />
      <rect x="6" y="3" width="52" height="50" rx="13" fill="none" stroke={`url(#${id}-h)`} strokeWidth="1.6" clipPath={`url(#${id}-c)`} />
      <rect x="10.5" y="7.5" width="43" height="41" rx="9.5" fill={`url(#${id}-d)`} />
      <path fill="#fff4e8" transform="translate(32 28.4) scale(1.2) translate(-32.04 -30.64)" d={LEGEND_H} />
      <rect x="46.8" y="10.6" width="3.2" height="7.6" rx="1.6" fill="#ffd2b0" filter={`url(#${id}-g)`} />
      <rect x="46.1" y="9.9" width="4.6" height="9" rx="2.3" fill="#a8411a" opacity=".55" />
      <rect x="46.8" y="10.6" width="3.2" height="7.6" rx="1.6" fill="#fff8ef" />
    </svg>
  );
}

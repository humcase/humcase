import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import '../index.css';
import { KitShowcase } from './KitShowcase';

// Development-only showcase: `kit.html` is not a production build entry.
if (!import.meta.env.DEV) throw new Error('The kit showcase only exists in development.');

createRoot(document.getElementById('root')!).render(<StrictMode><KitShowcase /></StrictMode>);

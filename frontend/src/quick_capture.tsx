import './utils/legacyStorage';
import React from 'react';
import ReactDOM from 'react-dom/client';
import QuickCaptureApp from './QuickCaptureApp';
import './index.css';
import './i18n';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QuickCaptureApp />
  </React.StrictMode>
);

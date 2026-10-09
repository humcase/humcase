import './utils/legacyStorage';
import React from 'react';
import ReactDOM from 'react-dom/client';
import AssistantMiniApp from './AssistantMiniApp';
import './index.css';
import './i18n';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AssistantMiniApp />
  </React.StrictMode>
);

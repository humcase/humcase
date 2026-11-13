import './utils/legacyStorage'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@fontsource/dm-mono/400.css'
import '@fontsource/dm-mono/500.css'
import './index.css'
import i18n from './i18n'
import { invoke } from '@tauri-apps/api/core'
import App from './App.tsx'
import { loadImagesFolder } from './utils/imageRefs'

// The engine (native menus, error messages) follows the interface language. Outside
// `i18n/index.ts` so as not to add an engine call in tests.
const forwardLanguage = (language: string) => {
  void invoke('set_ui_language', { language: language }).catch(() => {})
}
forwardLanguage(i18n.language)
i18n.on('languageChanged', forwardLanguage)

// Files dropped outside a place that takes them (the board, a sketch) are ignored: the window
// would otherwise open the file in place of the app (HTML drops are on, see `tauri.conf.json`).
const carriesFiles = (event: DragEvent) => event.dataTransfer?.types.includes('Files') ?? false
window.addEventListener('dragover', event => { if (carriesFiles(event)) event.preventDefault() })
window.addEventListener('drop', event => { if (carriesFiles(event)) event.preventDefault() })

// The images folder is read before the first render: the portable references of notes
// (`humcase-image://`) are displayed as soon as it opens.
void loadImagesFolder().finally(() => {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  )
})

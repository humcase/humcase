; Hooks of the NSIS installer (tauri.conf.json, bundle.windows.nsis.installerHooks).
;
; The template (Tauri CLI 2.12 and later) closes, through the Windows Restart Manager, only the
; processes running $INSTDIR\humcase.exe: the copy being replaced, its interface and the MCP servers
; started from it. A copy run from another folder (the repository's Humcase.exe) is left alone.
; The search-by-meaning workers, $INSTDIR\meaning\humcase-sense.exe, are separate programs that hold
; meaning\: close them the same way, by their path, after their Humcase (which would start them
; again), so that meaning\ can be replaced or removed. The template's own check after this hook
; then finds nothing left.

!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  !insertmacro CheckIfAppIsRunning "$INSTDIR\meaning\humcase-sense.exe" "${PRODUCTNAME}"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  !insertmacro CheckIfAppIsRunning "$INSTDIR\meaning\humcase-sense.exe" "${PRODUCTNAME}"
!macroend

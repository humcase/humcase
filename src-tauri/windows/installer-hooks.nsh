; Hooks of the NSIS installer (tauri.conf.json, bundle.windows.nsis.installerHooks).

; Before files are copied: a running Humcase (the interface, or an MCP server an agent started)
; keeps its search-by-meaning worker, meaning\humcase-sense.exe, which holds the worker and its
; runtime. The installer then closes Humcase.exe through the Restart Manager, but that closes only
; Humcase.exe itself: stop the workers of this folder first so that meaning\ can be replaced.
; Workers of another folder (a copy built elsewhere) are left alone. A worker stopped this way is
; started again by the next search of a Humcase still running.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec `powershell -NoProfile -NonInteractive -Command "Get-Process humcase-sense -ErrorAction SilentlyContinue | Where-Object { $$_.Path -like '$INSTDIR\*' } | Stop-Process -Force"`
  Pop $0
!macroend

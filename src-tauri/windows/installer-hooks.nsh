; Hooks of the NSIS installer (tauri.conf.json, bundle.windows.nsis.installerHooks).

; Before files are copied. Right after this hook, the installer kills by name every Humcase.exe
; of the current user (the interface and the MCP servers agents started, whatever their folder);
; their search-by-meaning workers, humcase-sense.exe, are separate programs that hold meaning\.
; Stop them the same way, with the installer's own plugin, so that meaning\ can be replaced.
!macro NSIS_HOOK_PREINSTALL
  nsis_tauri_utils::KillProcessCurrentUser "humcase-sense.exe"
  Pop $R0
!macroend

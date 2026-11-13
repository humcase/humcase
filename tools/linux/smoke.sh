#!/usr/bin/env bash
# Smoke test of the Linux bundles, run INSIDE the build container (see `build.sh smoke`).
# Everything happens under an isolated HOME: the data folder is $HOME/.humcase of a throwaway
# directory, never real data. No AI call is made (no key is configured).
#   smoke.sh cli          install the .deb, run `tools`, `handoff`, `add`, `list` and the MCP server
#   smoke.sh ui [click]   start Xvfb + openbox, launch the AppImage, take screenshots
#                         (optional "x,y" to click once between two screenshots)
set -euo pipefail

OUT=/out
SHOTS=$OUT/screenshots
export HOME=/tmp/smoke-home
mkdir -p "$HOME" "$SHOTS"

install_deb() {
  command -v humcase >/dev/null 2>&1 && return
  apt-get install -y --no-install-recommends "$OUT"/*.deb >/dev/null 2>&1 || apt-get install -y "$OUT"/*.deb
}

mcp_session() {
  # Newline-delimited JSON-RPC over stdio; the server answers one line per request.
  {
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
    echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_or_create_inbox","arguments":{"space":"private"}}}'
    sleep 1
    echo "$1"
    sleep 1
    echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_notes","arguments":{}}}'
    sleep 1
  } | humcase mcp 2>/tmp/mcp.err
}

case "${1:?cli|ui}" in
  cli)
    install_deb
    echo "== binary"; command -v humcase; humcase --version || true
    echo "== humcase tools (first lines)"; humcase tools | sed -n 1,5p
    echo "== humcase handoff"; humcase handoff | sed -n 1,15p
    echo "== inbox"; humcase call get_or_create_inbox --args "{\"space\":\"private\"}" | tee /tmp/inbox.json
    INBOX_ID=$(grep -oE "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}" /tmp/inbox.json | sed -n 1p)
    echo "== humcase add"; humcase add "Prepare the Linux release" -t todo -p "$INBOX_ID"
    echo "== humcase list"; humcase list
    echo "== MCP"
    echo "inbox/project id: ${INBOX_ID:-unknown}"
    ADD='{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"add_note","arguments":{"project_id":"'"${INBOX_ID}"'","title":"Added over MCP on Linux","note_type":"idea"}}}'
    mcp_session "$ADD" | cut -c1-400 | tee /tmp/mcp.out
    echo "-- stderr"; head -5 /tmp/mcp.err
    echo "== data folder"; ls -la "$HOME/.humcase"
    ;;
  ui)
    install_deb
    export DISPLAY=:99
    Xvfb :99 -screen 0 1920x1080x24 >/tmp/xvfb.log 2>&1 &
    sleep 2
    export XDG_RUNTIME_DIR=/tmp/xdg; mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
    openbox >/tmp/openbox.log 2>&1 &
    sleep 1
    # Seed one case through the CLI so a case can be opened.
    humcase add "Prepare the Linux release" -t todo -p "$(humcase call get_or_create_inbox --args "{\"space\":\"work\"}" | grep -oE "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}" | sed -n 1p)" >/dev/null
    export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1
    APPIMAGE=$(ls "$OUT"/*.AppImage | head -1)
    chmod +x "$APPIMAGE"
    APPIMAGE_EXTRACT_AND_RUN=1 dbus-run-session -- "$APPIMAGE" >/tmp/app.log 2>&1 &
    sleep 25
    import -window root "$SHOTS/01-home-with-case.png"
    xdotool search --name Humcase getwindowname %@ 2>/dev/null | head -3 || true
    n=2
    for pt in $(echo "${2:-}" | tr ";" " "); do
      xdotool mousemove "${pt%,*}" "${pt#*,}" click 1
      sleep 4
      import -window root "$SHOTS/0$n-step-$n.png"
      n=$((n+1))
    done
    echo "== app log (tail)"; tail -15 /tmp/app.log
    ;;
esac

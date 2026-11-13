"""Drives the installed Humcase window through Windows UI Automation, like a user with mouse and keyboard.
The release build has DevTools off, so CDP is not available: real clicks and keystrokes instead.
Usage: python ui.py <outDir> <stepName> [script.json]"""
import json, os, sys, time
from pywinauto import Desktop, keyboard
from PIL import ImageGrab

out_dir, step_name = sys.argv[1], sys.argv[2]
steps = json.load(open(sys.argv[3], encoding='utf-8')) if len(sys.argv) > 3 else [{"snap": "screen"}]
os.makedirs(out_dir, exist_ok=True)
log = []

def window():
    for _ in range(30):
        wins = [w for w in Desktop(backend='uia').windows() if w.window_text() == 'Humcase']
        if wins:
            return wins[0]
        time.sleep(1)
    raise RuntimeError('no Humcase window')

n = 0
def snap(label):
    global n
    n += 1
    base = os.path.join(out_dir, f"{step_name}-{n:02d}-{label}")
    ImageGrab.grab().save(base + '.png')
    w = window()
    lines = []
    for e in w.descendants():
        try:
            name = e.window_text().strip().replace('\n', ' ')
            ct = e.element_info.control_type
            if name and e.is_visible():
                lines.append(f"{ct}: {name[:100]}")
        except Exception:
            pass
    open(base + '.txt', 'w', encoding='utf-8').write('\n'.join(lines))
    print('snap', base, len(lines), 'elements')

w = window()
w.set_focus()
try:
    w.maximize()
except Exception:
    pass
time.sleep(1)
for s in steps:
    try:
        if 'click' in s:
            w = window()
            matches = [e for e in w.descendants(title=s['click']) if e.is_visible()]
            if not matches:
                matches = [e for e in w.descendants() if e.is_visible() and s['click'].lower() in e.window_text().lower()]
            if not matches:
                raise RuntimeError(f"nothing named {s['click']!r}")
            matches[s.get('nth', 0)].click_input()
        if 'type' in s:
            keyboard.send_keys(s['type'], with_spaces=True, pause=0.02)
        if 'keys' in s:
            keyboard.send_keys(s['keys'])
        time.sleep(s.get('wait', 1.5))
        if 'snap' in s:
            snap(s['snap'])
    except Exception as e:
        log.append(f"step {json.dumps(s)} failed: {e}")
        snap('failed')
open(os.path.join(out_dir, f"{step_name}-log.txt"), 'w', encoding='utf-8').write('\n'.join(log) or 'all steps ran')
print('\n'.join(log) or 'all steps ran')

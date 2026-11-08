# Monkey Mascot 🐒

A little monkey that lives on top of all your windows, never steals focus, and
runs on Linux (X11) and Windows from one PySide6 codebase. It has no image
assets: every frame is drawn procedurally from a `Pose`.

It walks around, scratches its head, eats bananas, jumps, looks at your cursor
and blinks. You can poke it (click), and you can grab it and throw it (drag and
release). A hard landing leaves it dizzy.

## Run

```bash
python -m venv .venv
.venv/bin/pip install -e .          # Windows: .venv\Scripts\pip install -e .
.venv/bin/python -m monkey_mascot   # or: monkey-mascot
```

Options: `--scale 1.5` for a bigger monkey. `--managed` (Linux only) lets the
window manager manage the window instead of bypassing it (see below).

**Controls:** left-click to poke, drag to carry and throw. Right-click the
monkey, or use the tray icon, for **Stay put**, **Click-through** and **Quit**.
Ctrl+C in the terminal also quits.

## Layout

| File | What it does |
|---|---|
| `monkey_mascot/pose.py` | `Pose`: one frame of body language (limb angles, eyes, mouth, banana…) |
| `monkey_mascot/brain.py` | Behaviour state machine and physics. Pure Python, no Qt |
| `monkey_mascot/monkey.py` | Procedural renderer: `paint_monkey(painter, pose, facing)` |
| `monkey_mascot/window.py` | The transparent, topmost, no-focus window, input handling and hit mask |
| `monkey_mascot/app.py` | Entry point: CLI arguments, tray icon and menu |
| `tools/preview.py` | Renders every animation to a PNG contact sheet, without opening a window |
| `tests/test_brain.py` | Headless behaviour tests: `python -m unittest discover tests` |

To add a behaviour, add a state method `_mystate(self, dt, bounds, pose)` in
`brain.py` that fills in the `Pose`, and add it to `_pick_next`. To change the
look, edit `monkey.py`, then run `python tools/preview.py` to see every pose at
once.

## How it stays on top without taking focus

| | Linux X11 | Windows |
|---|---|---|
| On top | override-redirect window (`X11BypassWindowManagerHint`) | `WS_EX_TOPMOST`, re-asserted every 2s |
| No focus | override-redirect windows are never focused | `WS_EX_NOACTIVATE` (`WindowDoesNotAcceptFocus`) |
| Not in taskbar/Alt+Tab | not managed by the WM at all | `WS_EX_TOOLWINDOW` (`Qt.Tool`) |
| Clicks pass around the monkey | window shape (XShape) updated every frame from the alpha | per-pixel-alpha layered window: fully transparent pixels already pass clicks through |
| Click-through mode | empty input shape | `WS_EX_TRANSPARENT` |

**Wayland:** apps can't position themselves or stay on top there, so the app
forces XWayland (`QT_QPA_PLATFORM=xcb`) automatically.

**Fullscreen games** in exclusive mode will cover the monkey. Borderless
windowed games won't.

## Ship a standalone app

```bash
pip install pyinstaller
pyinstaller --noconsole --onefile -n MonkeyMascot monkey_mascot/__main__.py
```

Build on each OS for that OS. PyInstaller doesn't cross-compile.

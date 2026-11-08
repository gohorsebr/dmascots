# Desktop Mascots

Little characters that live on top of all your windows and never steal focus.
They run on Linux (X11) and Windows from one PySide6 codebase, with no image
assets: every frame is drawn procedurally from a pose.

| Mascot | What it gets up to |
|---|---|
| 🐒 **monkey** | walks around, scratches its head, eats bananas, hops |

All mascots look at your cursor and blink. You can poke them (click), and you
can grab and throw them (drag and release). A hard landing leaves them dizzy.

## Run

```bash
python -m venv .venv
.venv/bin/pip install -e .          # Windows: .venv\Scripts\pip install -e .
.venv/bin/desktop-mascots           # one of each mascot
.venv/bin/desktop-mascots monkey monkey --scale 1.5
```

`--managed` (Linux only) lets the window manager manage the windows instead of
bypassing it (see below).

**Controls:** left-click to poke, drag to carry and throw. Right-click a mascot,
or use the tray icon, for **Add** (spawn another), **Shoo** (remove that one),
**Stay put**, **Click-through** and **Quit**. Ctrl+C in the terminal also quits.

## Layout

| Path | What it does |
|---|---|
| `desktop_mascots/engine.py` | `Brain` base class: physics, the shared states (idle, walk, jump, fall, land, dizzy, dragged), blinking, gaze and speech bubbles. Pure Python, no Qt |
| `desktop_mascots/art.py` | Shared painting helpers: outlined strokes, speech bubble, dizzy stars, icons |
| `desktop_mascots/window.py` | The transparent, topmost, no-focus window, input handling and hit mask |
| `desktop_mascots/app.py` | CLI, tray icon and menus, and the herd of windows |
| `desktop_mascots/characters/<name>/` | One mascot: `brain.py` (pose and behaviours), `art.py` (renderer), `__init__.py` (`CHARACTER` descriptor) |
| `tools/preview.py` | Renders every mascot's animations to a PNG contact sheet, without opening a window |
| `tests/test_mascots.py` | Headless behaviour tests, run for every mascot: `python -m unittest discover tests` |

### Adding a mascot

1. Create `characters/<name>/brain.py`:
   - a `Pose` dataclass that extends `BasePose`;
   - a `Brain` subclass that sets `CHOICES`, `LINES` and `WALK_SPEED`;
   - the `pose_*` hooks for the shared states;
   - any states of its own, as `_<state>(self, dt, bounds, pose)`.
2. Create `characters/<name>/art.py` with `paint(painter, pose, facing, head_only=False)`.
   The origin is between the feet, y points down, and the mascot faces +x.
3. Expose a `CHARACTER = Character(...)` and register it in `characters/__init__.py`.
4. Run `python tools/preview.py` to see every pose at once, then run the tests.

## How it stays on top without taking focus

| | Linux X11 | Windows |
|---|---|---|
| On top | override-redirect window (`X11BypassWindowManagerHint`) | `WS_EX_TOPMOST`, re-asserted every 2s |
| No focus | override-redirect windows are never focused | `WS_EX_NOACTIVATE` (`WindowDoesNotAcceptFocus`) |
| Not in taskbar/Alt+Tab | not managed by the WM at all | `WS_EX_TOOLWINDOW` (`Qt.Tool`) |
| Clicks pass around the mascot | window shape (XShape) updated every frame from the alpha | per-pixel-alpha layered window: fully transparent pixels already pass clicks through |
| Click-through mode | empty input shape | `WS_EX_TRANSPARENT` |

**Wayland:** apps can't position themselves or stay on top there, so the app
forces XWayland (`QT_QPA_PLATFORM=xcb`) automatically.

**Fullscreen games** in exclusive mode will cover the mascots. Borderless
windowed games won't.

## Ship a standalone app

```bash
pip install pyinstaller
pyinstaller --noconsole --onefile -n DesktopMascots desktop_mascots/__main__.py
```

Build on each OS for that OS. PyInstaller doesn't cross-compile.

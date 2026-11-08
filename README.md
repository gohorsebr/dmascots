# Desktop Mascots

Little characters that live on top of all your windows and never steal focus.
They run on Linux (X11) and Windows from one PySide6 codebase, with no image
assets: every frame is drawn procedurally from a pose.

| Mascot | What it gets up to |
|---|---|
| 🐒 **monkey** | walks around, scratches its head, eats bananas, hops |
| 🪿 **goose** | waddles, grazes, preens, honks back when poked, charges at your cursor hissing |
| 🐦‍⬛ **crow** | struts and hops, pecks, caws, tilts its head at your cursor, flies across the screen, hoards shiny coins |
| 🐕 **labrador** | walks, sniffs around, sits, begs for treats, naps, play-bows into zoomies, barks and wags when poked |
| 🐕 **collie** | a border collie: same dog, more energy, and it "herds" your cursor with a low, staring stalk before dashing off |

All mascots look at your cursor and blink. You can poke them (click), and you
can grab and throw them (drag and release). A hard landing leaves them dizzy.

## Run

```bash
make install                        # venv + editable install (or: pip install -e .)
make run                            # one of each mascot
make run ARGS="goose --time 5 --say hi"
make test / make preview / make build / make clean   # `make` lists everything
```

Or call the installed command directly:

```bash
.venv/bin/desktop-mascots           # one of each mascot
.venv/bin/desktop-mascots goose goose --scale 1.5
.venv/bin/desktop-mascots goose --time 5 --say "Build finished!"   # pop in, say it, leave
```

| Option | What it does |
|---|---|
| `--time SECONDS` | The mascots leave after this many seconds. If the app was started this way, it exits once they're gone |
| `--say TEXT` | The mascots start by saying `TEXT` (`-` reads it from stdin). With `--time`, the bubble stays up until they leave |
| `--scale N` | Size multiplier |
| `--managed` | Linux only: let the window manager manage the windows instead of bypassing it (see below) |

`--time` and `--say` together make a quick desktop notification, e.g.
`make; desktop-mascots goose --time 6 --say "make exited with $?"`.

**Controls:** left-click to poke, drag to carry and throw. Right-click a mascot,
or use the tray icon, for **Say something…**, **Add** (spawn another),
**Shoo** (remove that one), **Stay put**, **Click-through** and **Quit**.
Ctrl+C in the terminal also quits.

Only one instance runs at a time. Running `desktop-mascots goose` again adds a
goose to the mascots already on screen. `--time` and `--say` apply to the
mascots that command adds, so a timed goose leaves without closing the others.

## Make them talk

```bash
desktop-mascots say "Lunch time!"                    # every mascot says it
desktop-mascots say --to goose "HONK means hello"    # only the geese
desktop-mascots say --seconds 10 "stays up for 10s"  # default: based on length
make && desktop-mascots say "build passed" || desktop-mascots say --to goose "BUILD FAILED"
git log -1 --format=%s | desktop-mascots say -       # '-' reads stdin
```

Long text wraps, up to 5 lines, then gets cut with `…`. A message you send
isn't replaced by the mascots' own chatter (pokes, honks) while it's showing.
The command exits non-zero if the app isn't running, or if no mascot of that
kind is on screen, so it works in scripts.

It talks to the running app over a local socket: a Unix socket on Linux, a
named pipe on Windows. Only your own user can connect to it. The protocol is
one JSON object per line (see `desktop_mascots/ipc.py`), so anything can send
messages, not just the CLI. On Windows the `desktop-mascots` launcher is a GUI
script and prints nothing, so use `python -m desktop_mascots say ...` there to
see errors.

## Layout

| Path | What it does |
|---|---|
| `desktop_mascots/engine.py` | `Brain` base class: physics, the shared states (idle, walk, jump, fall, land, dizzy, dragged), blinking, gaze and speech bubbles. Pure Python, no Qt |
| `desktop_mascots/art.py` | Shared painting helpers: outlined strokes, speech bubble, dizzy stars, icons |
| `desktop_mascots/window.py` | The transparent, topmost, no-focus window, input handling and hit mask |
| `desktop_mascots/app.py` | CLI (including `say`), tray icon and menus, and the herd of windows |
| `desktop_mascots/ipc.py` | Per-user local socket that the `say` command and a second launch use to reach the running app |
| `desktop_mascots/quadruped/` | Shared four-legged rig: poses give shoulder/hip positions and foot targets, and the painter solves the knees with two-bone IK, so walk/trot/gallop/sit/lie/bow work for any species |
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

Four-legged animals can start from the rig in `quadruped/`. Subclass `QuadBrain`
with a `Rig` (joint positions and bone lengths) and `QuadArt` (coat colours, head,
tail and markings), and they get gaits, sitting, lying down, play bows, zoomies
and the airborne poses for free. The dogs, cat and horse are all built this way.
Flying birds add `"fly"` to `CHOICES` and `AIRBORNE` and implement `pose_flight`.

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
make build      # -> dist/DesktopMascots (a single ~60 MB file, PyInstaller)
```

Build on each OS for that OS. PyInstaller doesn't cross-compile.

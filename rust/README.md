# Desktop Mascots: Rust port

A port of the Python/PySide6 app in `../desktop_mascots` that builds to **one file
with no runtime dependencies**, and **cross-compiles from Linux to Windows**.

| | Python (PySide6 + PyInstaller) | Rust |
|---|---|---|
| Linux binary | ~60 MB onefile, unpacks on every start | 2.3 MB, needs only libc (no Qt, no libX11/libxcb) |
| Windows binary | build on Windows | 2.1 MB `.exe` built on Linux, imports only system DLLs |
| RAM, 3 mascots | ~100 MB | ~6 MB |

## Status

| Part | State |
|---|---|
| Drawing, engine, all 8 mascots (monkey, goose, crow, parrot, labrador, collie, cat, horse + the quadruped IK rig) | done; preview sheets match the Python ones |
| Tests: `tests/behaviour.rs` (test_mascots.py), `tests/herd.rs` (test_herd.py, headless backend) | 17 passing |
| CLI: start/hand-off, `list [--json]`, `add`, `remove`, `say`, `steal`, `quit`; ids and targets; `--click-through`, `--menu`, `--no-tray`, `--time`, `--say`, `--scale`, `--managed` | done, tested on X11 |
| Linux/X11: windows, XShape, drag/throw/poke, tray (StatusNotifierItem via ksni, pure Rust), right-click menu (self-drawn popup), "Say something…" (zenity/kdialog/yad) | done, tested on a live desktop |
| Windows: layered windows, tray + native menus, popup menu, prompt dialog, named pipe | type-checks and links (3.1 MB exe, system DLLs only); **not run on Windows yet** |
| Menu icons (mascot heads next to "Add" entries) | not done |

## Run

```bash
make run ARGS="goose --time 5 --say hi"     # or: cargo run --release -- goose
make test                                    # headless behaviour tests
make preview                                 # every animation -> preview.png
```

Testing next to a running instance? Give the test its own socket so it never
reaches (or quits) the mascots you already have on screen:

```bash
DESKTOP_MASCOTS_SOCKET=dm-test-$$ cargo run -- goose --time 5
```

## Build

```bash
make build            # target/release/desktop-mascots for this OS
make build-windows    # target/x86_64-pc-windows-gnu/release/desktop-mascots.exe
```

`build-windows` uses mingw-w64 if it's installed (`sudo apt install mingw-w64`).
If not, it links with zig instead, through the shims in `tools/`. No root needed:

```bash
pip install --user ziglang
make build-windows ZIG="python3 -m ziglang"
```

The `.exe` is a GUI-subsystem program, so no console window opens. When it runs
from a terminal it attaches to that console, so `desktop-mascots.exe say hi`
and `--help` still print there.

## Layout

| Path | Port of | What it does |
|---|---|---|
| `src/gfx.rs` | (QtGui) | `Painter`, `Path`, `Xf`, `Pen`, `Color`, text: the subset of QPainter the art uses, same semantics (degrees, y down, Qt transform order) |
| `src/engine.rs` | `engine.py` | `Core` (the Python `Brain`'s state), `Species` trait (pose hooks, own states, painter), `Brain<S>`, `Character` |
| `src/art.rs` | `art.py` | outlined strokes, tapered curves, stars, z's, speech bubbles, icons |
| `src/characters/<name>.rs` | `characters/<name>/` | one mascot: pose, constants, behaviour, art |
| `src/platform/` | `window.py` | `Backend` trait, `x11.rs`, `windows.rs` |
| `src/app.rs` | `window.py`, `app.py` | mascot windows, the herd, the main loop, socket requests |
| `src/ipc.rs` | `ipc.py` | control socket client and server |
| `src/main.rs` | `app.py` | CLI |
| `examples/preview.rs` | `tools/preview.py` | contact sheet PNG |

### Porting a mascot

Python → Rust is mostly mechanical:

- The `Pose` dataclass becomes `pose!(MonkeyPose { field: type = default, ... })`.
- The `Brain` subclass becomes a `Species` impl:
  - Class constants go in `const PARAMS: Params = Params { ..Params::BASE }`.
  - `pose_*` hooks become trait methods that take `c: &Core`.
  - The extra `_<state>` methods are dispatched from `Species::state` with a `match`.
- `self.clock` / `self.go(..)` / `self.say(..)` / `self.rng.choice(..)` become
  `c.clock` / `c.go(..)` / `c.say(.., false)` / `c.choice(..)`.
- QPainter calls keep their names in snake_case:
  - `p.setBrush(X)` becomes `p.set_brush(X)`.
  - `p.setPen(Qt.NoPen)` becomes `p.no_pen()`.
  - `QPointF(x, y)` becomes `pt(x, y)`.
- Register the mascot in `characters/mod.rs`. Then run `make preview` and diff it
  against `python tools/preview.py`.

The embedded font is Noto Sans Bold (SIL Open Font License 1.1).

"""Entry point: CLI, tray icon/menus, and the herd of mascot windows."""
from __future__ import annotations

import argparse
import os
import random
import signal
import sys

MAX_TEXT = 500


def reading_time(text: str) -> float:
    """How long a bubble stays up: long enough to read, never forever."""
    return min(15.0, max(2.5, 1.5 + 0.06 * len(text)))


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] == ["say"]:
        return say_command(argv[1:])
    if argv[:1] == ["quit"]:
        return quit_command(argv[1:])
    if argv[:1] == ["steal"]:
        return steal_command(argv[1:])

    from .characters import CHARACTERS

    names = ", ".join(CHARACTERS)
    parser = argparse.ArgumentParser(
        prog="desktop-mascots", description="Mascots that live on top of your windows.",
        epilog="Commands for the running app: 'say [--to MASCOT] TEXT', "
               "'steal [--to MASCOT] [--seconds N]', 'quit' (or Ctrl+C).")
    parser.add_argument("mascots", nargs="*", metavar="MASCOT",
                        help=f"mascots to spawn, repeat for more ({names}); default: one of each")
    parser.add_argument("--scale", type=float, default=1.0, help="size multiplier (default 1.0)")
    parser.add_argument("--time", type=float, metavar="SECONDS",
                        help="the mascots leave after this many seconds (and the app exits if none are left)")
    parser.add_argument("--say", metavar="TEXT",
                        help="the mascots start by saying TEXT ('-' reads it from stdin)")
    parser.add_argument("--menu", action="store_true",
                        help="right-clicking a mascot opens its menu (off by default)")
    parser.add_argument("--no-tray", action="store_true",
                        help="no tray icon; quit with Ctrl+C or 'desktop-mascots quit'")
    parser.add_argument("--managed", action="store_true",
                        help="Linux/X11: let the window manager manage the windows instead of bypassing it")
    args = parser.parse_args(argv)
    unknown = [m for m in args.mascots if m not in CHARACTERS]
    if unknown:
        parser.error(f"unknown mascot(s): {', '.join(unknown)} (available: {names})")
    if args.time is not None and args.time <= 0:
        parser.error("--time must be greater than 0")
    if args.say == "-":
        args.say = sys.stdin.read()

    # Wayland doesn't let clients position themselves or stay on top; use XWayland.
    if sys.platform.startswith("linux") and os.environ.get("WAYLAND_DISPLAY"):
        os.environ.setdefault("QT_QPA_PLATFORM", "xcb")

    from PySide6.QtWidgets import QApplication

    from .ipc import Server, send

    app = QApplication(sys.argv[:1])
    wanted = args.mascots or list(CHARACTERS)

    # Already running? Hand the new mascots to that instance instead of starting a second one.
    reply = send({"cmd": "spawn", "mascots": wanted, "lifetime": args.time, "say": args.say})
    if reply is not None:
        leaving = f" (leaving in {args.time:g}s)" if args.time else ""
        print(f"desktop-mascots is already running; added {', '.join(wanted)}{leaving}.")
        return 0

    app.setApplicationName("Desktop Mascots")
    app.setQuitOnLastWindowClosed(False)
    signal.signal(signal.SIGINT, lambda *_: app.quit())   # Ctrl+C in the terminal
    signal.signal(signal.SIGTERM, lambda *_: app.quit())  # `kill`: still exit cleanly

    herd = Herd(app, CHARACTERS, scale=args.scale, managed=args.managed,
                menu=args.menu, tray=not args.no_tray)
    herd.quit_when_empty = args.time is not None
    server = Server(herd.handle, app)
    if not server.listening:
        print("warning: couldn't open the control socket; 'say' won't reach this instance",
              file=sys.stderr)
    for name in wanted:
        herd.spawn(CHARACTERS[name], lifetime=args.time, say=args.say)
    return app.exec()


def steal_command(argv: list[str]) -> int:
    from .characters import CHARACTERS

    parser = argparse.ArgumentParser(
        prog="desktop-mascots steal",
        description="A mascot runs to your pointer, grabs it and runs off with it. "
                    "Click the thief to get it back early.")
    parser.add_argument("--to", default="any", metavar="MASCOT",
                        help=f"who steals it: any (default, picked at random), {', '.join(CHARACTERS)}")
    parser.add_argument("--seconds", type=float, default=5.0,
                        help="how long it holds on to the pointer (default 5, max 60)")
    args = parser.parse_args(argv)
    if args.to != "any" and args.to not in CHARACTERS:
        parser.error(f"unknown mascot: {args.to}")
    if not 0 < args.seconds <= 60:
        parser.error("--seconds must be more than 0 and at most 60")

    from PySide6.QtCore import QCoreApplication

    from .ipc import send

    _app = QCoreApplication(sys.argv[:1])  # noqa: F841 -- QLocalSocket needs an app object
    reply = send({"cmd": "steal", "to": args.to, "seconds": args.seconds})
    if reply is None:
        print("desktop-mascots isn't running (start it with: desktop-mascots)", file=sys.stderr)
        return 1
    if not reply.get("ok"):
        print(f"error: {reply.get('error')}", file=sys.stderr)
        return 1
    return 0


def quit_command(argv: list[str]) -> int:
    argparse.ArgumentParser(prog="desktop-mascots quit",
                            description="Close the running mascots.").parse_args(argv)
    from PySide6.QtCore import QCoreApplication

    from .ipc import send

    _app = QCoreApplication(sys.argv[:1])  # noqa: F841 -- QLocalSocket needs an app object
    if send({"cmd": "quit"}) is None:
        print("desktop-mascots isn't running", file=sys.stderr)
        return 1
    return 0


def say_command(argv: list[str]) -> int:
    from .characters import CHARACTERS

    parser = argparse.ArgumentParser(prog="desktop-mascots say",
                                     description="Make the running mascots say something.")
    parser.add_argument("text", nargs="+", help="what to say ('-' reads it from stdin)")
    parser.add_argument("--to", default="all", metavar="MASCOT",
                        help=f"who says it: all (default), {', '.join(CHARACTERS)}")
    parser.add_argument("--seconds", type=float, help="how long the bubble stays (default: based on length)")
    args = parser.parse_args(argv)
    if args.to != "all" and args.to not in CHARACTERS:
        parser.error(f"unknown mascot: {args.to}")
    text = sys.stdin.read() if args.text == ["-"] else " ".join(args.text)
    text = " ".join(text.split())  # one paragraph; the bubble wraps it
    if not text:
        parser.error("nothing to say")

    from PySide6.QtCore import QCoreApplication

    from .ipc import send

    _app = QCoreApplication(sys.argv[:1])  # noqa: F841 -- QLocalSocket needs an app object
    reply = send({"cmd": "say", "text": text, "to": args.to, "seconds": args.seconds})
    if reply is None:
        print("desktop-mascots isn't running (start it with: desktop-mascots)", file=sys.stderr)
        return 1
    if not reply.get("ok"):
        print(f"error: {reply.get('error')}", file=sys.stderr)
        return 1
    if reply.get("count", 0) == 0:
        print(f"no {args.to} on screen right now", file=sys.stderr)
        return 1
    return 0


class Herd:
    """All live mascot windows plus the menus that control them."""

    def __init__(self, app, characters, scale: float, managed: bool, menu: bool = False, tray: bool = True):
        from PySide6.QtGui import QAction, QIcon
        from PySide6.QtWidgets import QMenu, QSystemTrayIcon

        from .art import render_icon

        self.app, self.characters, self.scale, self.managed = app, characters, scale, managed
        self.menu_enabled = menu      # right-click menu on the mascots themselves
        self.windows: list = []
        self.quit_when_empty = False   # set when started with --time: exit once all have left

        self.add_menu = QMenu("Add")
        self.icons = {}
        for name, char in characters.items():
            self.icons[name] = QIcon(render_icon(char))
            act = self.add_menu.addAction(self.icons[name], name.capitalize())
            act.triggered.connect(lambda _=False, c=char: self.spawn(c))

        self.stay = QAction("Stay put", app, checkable=True)
        self.stay.toggled.connect(lambda on: [w.set_paused(on) for w in self.windows])
        self.through = QAction("Click-through", app, checkable=True)
        self.through.toggled.connect(lambda on: [w.set_click_through(on) for w in self.windows])
        self.quit = QAction("Quit", app)
        self.quit.triggered.connect(app.quit)

        first = next(iter(self.icons.values()))
        app.setWindowIcon(first)
        self.tray = None
        if tray and QSystemTrayIcon.isSystemTrayAvailable():
            self.tray_menu = QMenu()
            self.tray_menu.addAction("Say something…", lambda: self.ask_and_say(self.windows))
            self._add_shared(self.tray_menu)
            self.tray = QSystemTrayIcon(first)
            self.tray.setToolTip("Desktop Mascots")
            self.tray.setContextMenu(self.tray_menu)
            self.tray.show()

    def _add_shared(self, menu) -> None:
        menu.addMenu(self.add_menu)
        menu.addAction(self.stay)
        menu.addAction(self.through)
        menu.addSeparator()
        menu.addAction(self.quit)

    def spawn(self, character, lifetime: float | None = None, say: str | None = None):
        """Show a new mascot. lifetime: seconds until it leaves; say: its opening line."""
        from PySide6.QtCore import QTimer
        from PySide6.QtWidgets import QMenu

        from .window import MascotWindow

        w = MascotWindow(character, scale=self.scale, managed=self.managed)
        if self.menu_enabled:     # otherwise right-clicks are simply ignored
            w.menu = QMenu(w)
            w.menu.addAction("Say something…", lambda: self.ask_and_say([w]))
            w.menu.addAction(self.icons[character.name], f"Shoo the {character.name}", lambda: self.remove(w))
            w.menu.addSeparator()
            self._add_shared(w.menu)
        w.set_paused(self.stay.isChecked())
        w.show()
        w.set_click_through(self.through.isChecked())
        self.windows.append(w)
        if say:
            # with a lifetime the opening line stays up until the mascot leaves
            text = " ".join(say.split())[:MAX_TEXT]
            self.say(text, [w], lifetime or reading_time(text))
        if lifetime:
            # parented to the window: cancelled automatically if it's shooed first
            QTimer.singleShot(round(lifetime * 1000), w, lambda: self.remove(w))
        return w

    def remove(self, w) -> None:
        if w not in self.windows:
            return
        self.windows.remove(w)
        w.close()
        w.deleteLater()
        if not self.windows and (self.tray is None or self.quit_when_empty):
            self.app.quit()  # nothing left to click on, or our --time is up

    def say(self, text: str, windows, seconds: float | None = None) -> int:
        text = " ".join(text.split())[:MAX_TEXT]
        if not text:
            return 0
        for w in windows:
            w.brain.say(text, seconds or reading_time(text), important=True)
        return len(windows)

    def ask_and_say(self, windows) -> None:
        from PySide6.QtWidgets import QInputDialog

        text, ok = QInputDialog.getText(None, "Desktop Mascots", "Say:")
        if ok:
            self.say(text, [w for w in windows if w in self.windows])

    def handle(self, request: dict) -> dict:
        """Requests from the control socket (see ipc.py)."""
        cmd = request.get("cmd")
        if cmd == "say":
            to = request.get("to") or "all"
            targets = [w for w in self.windows if to == "all" or w.character.name == to]
            seconds = request.get("seconds")
            count = self.say(str(request.get("text", "")), targets,
                             float(seconds) if seconds else None)
            return {"ok": True, "count": count}
        if cmd == "spawn":
            names = [n for n in request.get("mascots", []) if n in self.characters]
            lifetime = request.get("lifetime")
            lifetime = float(lifetime) if lifetime and float(lifetime) > 0 else None
            say = request.get("say")
            for n in names:
                self.spawn(self.characters[n], lifetime=lifetime, say=str(say) if say else None)
            return {"ok": True, "count": len(names)}
        if cmd == "steal":
            if any(w.brain.state == "steal" or w.brain.pointer or w.brain._pending_steal is not None
                   for w in self.windows):
                return {"ok": False, "error": "someone is already running off with it"}
            to = request.get("to") or "any"
            thieves = [w for w in self.windows
                       if (to == "any" or w.character.name == to) and w.brain.state != "dragged"]
            if not thieves:
                return {"ok": False, "error": f"no {to if to != 'any' else 'mascot'} available to steal it"}
            seconds = float(request.get("seconds") or 5.0)
            random.choice(thieves).brain.steal(seconds)
            return {"ok": True, "count": 1}
        if cmd == "quit":
            from PySide6.QtCore import QTimer
            QTimer.singleShot(0, self.app.quit)   # after the reply has gone out
            return {"ok": True, "count": len(self.windows)}
        return {"ok": False, "error": f"unknown command: {cmd!r}"}


if __name__ == "__main__":
    raise SystemExit(main())

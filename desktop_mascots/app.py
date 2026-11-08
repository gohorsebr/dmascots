"""Entry point: CLI, tray icon/menus, and the herd of mascot windows."""
from __future__ import annotations

import argparse
import os
import signal
import sys


def main(argv: list[str] | None = None) -> int:
    from .characters import CHARACTERS

    names = ", ".join(CHARACTERS)
    parser = argparse.ArgumentParser(prog="desktop-mascots",
                                     description="Mascots that live on top of your windows.")
    parser.add_argument("mascots", nargs="*", metavar="MASCOT",
                        help=f"mascots to spawn, repeat for more ({names}); default: one of each")
    parser.add_argument("--scale", type=float, default=1.0, help="size multiplier (default 1.0)")
    parser.add_argument("--managed", action="store_true",
                        help="Linux/X11: let the window manager manage the windows instead of bypassing it")
    args = parser.parse_args(argv)
    unknown = [m for m in args.mascots if m not in CHARACTERS]
    if unknown:
        parser.error(f"unknown mascot(s): {', '.join(unknown)} (available: {names})")

    # Wayland doesn't let clients position themselves or stay on top; use XWayland.
    if sys.platform.startswith("linux") and os.environ.get("WAYLAND_DISPLAY"):
        os.environ.setdefault("QT_QPA_PLATFORM", "xcb")

    from PySide6.QtWidgets import QApplication

    app = QApplication(sys.argv[:1])
    app.setApplicationName("Desktop Mascots")
    app.setQuitOnLastWindowClosed(False)
    signal.signal(signal.SIGINT, lambda *_: app.quit())  # Ctrl+C in the terminal

    herd = Herd(app, CHARACTERS, scale=args.scale, managed=args.managed)
    for name in args.mascots or CHARACTERS:
        herd.spawn(CHARACTERS[name])
    return app.exec()


class Herd:
    """All live mascot windows plus the menus that control them."""

    def __init__(self, app, characters, scale: float, managed: bool):
        from PySide6.QtGui import QAction, QIcon
        from PySide6.QtWidgets import QMenu, QSystemTrayIcon

        from .art import render_icon

        self.app, self.scale, self.managed = app, scale, managed
        self.windows: list = []

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
        if QSystemTrayIcon.isSystemTrayAvailable():
            self.tray_menu = QMenu()
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

    def spawn(self, character) -> None:
        from PySide6.QtWidgets import QMenu

        from .window import MascotWindow

        w = MascotWindow(character, scale=self.scale, managed=self.managed)
        w.menu = QMenu(w)
        w.menu.addAction(self.icons[character.name], f"Shoo the {character.name}", lambda: self.remove(w))
        w.menu.addSeparator()
        self._add_shared(w.menu)
        w.set_paused(self.stay.isChecked())
        w.show()
        w.set_click_through(self.through.isChecked())
        self.windows.append(w)

    def remove(self, w) -> None:
        self.windows.remove(w)
        w.close()
        w.deleteLater()
        if not self.windows and self.tray is None:
            self.app.quit()  # nothing left to click on


if __name__ == "__main__":
    raise SystemExit(main())

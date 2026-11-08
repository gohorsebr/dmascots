"""Entry point: sets up the Qt app, the tray icon/menu and the mascot window."""
from __future__ import annotations

import argparse
import os
import signal
import sys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="monkey-mascot", description="A monkey that lives on your desktop.")
    parser.add_argument("--scale", type=float, default=1.0, help="size multiplier (default 1.0)")
    parser.add_argument("--managed", action="store_true",
                        help="Linux/X11: let the window manager manage the window instead of bypassing it")
    args = parser.parse_args(argv)

    # Wayland doesn't let clients position themselves or stay on top; use XWayland.
    if sys.platform.startswith("linux") and os.environ.get("WAYLAND_DISPLAY"):
        os.environ.setdefault("QT_QPA_PLATFORM", "xcb")

    from PySide6.QtGui import QIcon
    from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

    from .monkey import render_icon
    from .window import MascotWindow

    app = QApplication(sys.argv[:1])
    app.setApplicationName("Monkey Mascot")
    app.setQuitOnLastWindowClosed(False)
    signal.signal(signal.SIGINT, lambda *_: app.quit())  # Ctrl+C in the terminal

    icon = QIcon(render_icon(64))
    app.setWindowIcon(icon)

    menu = QMenu()
    stay = menu.addAction("Stay put")
    stay.setCheckable(True)
    through = menu.addAction("Click-through")
    through.setCheckable(True)
    menu.addSeparator()
    menu.addAction("Quit", app.quit)

    win = MascotWindow(scale=args.scale, managed=args.managed, menu=menu)
    stay.toggled.connect(win.set_paused)
    through.toggled.connect(win.set_click_through)

    tray = None
    if QSystemTrayIcon.isSystemTrayAvailable():
        tray = QSystemTrayIcon(icon)
        tray.setToolTip("Monkey Mascot")
        tray.setContextMenu(menu)
        tray.show()

    win.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())

"""The transparent, always-on-top, never-focused window that hosts one mascot."""
from __future__ import annotations

import random
import sys

from PySide6.QtCore import QElapsedTimer, QPoint, QPointF, QSize, Qt, QTimer
from PySide6.QtGui import QBitmap, QCursor, QGuiApplication, QImage, QPainter, QRegion
from PySide6.QtWidgets import QMenu, QWidget

from .engine import Bounds, Character

FEET_FROM_BOTTOM = 6
FRAME_MS = 16


class MascotWindow(QWidget):
    def __init__(self, character: Character, scale: float = 1.0, managed: bool = False):
        flags = (Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
                 | Qt.WindowDoesNotAcceptFocus | Qt.NoDropShadowWindowHint)
        if sys.platform.startswith("linux") and not managed:
            # override-redirect: the WM never focuses, decorates, constrains or lists it
            flags |= Qt.X11BypassWindowManagerHint
        super().__init__(None, flags)
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.setAttribute(Qt.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WA_MacAlwaysShowToolWindow)
        self.setFocusPolicy(Qt.NoFocus)
        self.setWindowTitle(f"Desktop mascot: {character.name}")
        self.setCursor(Qt.OpenHandCursor)

        self.character = character
        self.s = scale
        self.base_h = character.size[1]
        self.setFixedSize(round(character.size[0] * scale), round(self.base_h * scale))
        self.menu: QMenu | None = None
        self.click_through = False
        # X11 hit-tests the whole rectangle, so shape the window to the mascot.
        # Windows layered windows already let clicks through fully transparent pixels.
        self._use_mask = QGuiApplication.platformName() == "xcb"
        self._frame = QImage()

        screen = QGuiApplication.primaryScreen()
        self._screen = screen
        g = screen.availableGeometry()
        margin = character.half_width * scale
        self.brain = character.brain(random.uniform(g.left() + margin, g.right() - margin),
                                     g.top() + character.height * scale)

        self._press: QPoint | None = None
        self._grab_offset = QPointF()
        self._dragging = False

        self._elapsed = QElapsedTimer()
        self._elapsed.start()
        self._timer = QTimer(self)
        self._timer.setTimerType(Qt.PreciseTimer)
        self._timer.timeout.connect(self._tick)
        self._timer.start(FRAME_MS)
        if sys.platform == "win32":
            self._topmost_timer = QTimer(self)
            self._topmost_timer.timeout.connect(self._reassert_topmost)
            self._topmost_timer.start(2000)
        self._tick()

    # ----------------------------------------------------------- loop
    def _bounds(self) -> Bounds:
        b = self.brain
        probe = QCursor.pos() if self._dragging else QPoint(int(b.x), int(b.y) - 5)
        self._screen = QGuiApplication.screenAt(probe) or self._screen
        g = self._screen.availableGeometry()
        m = self.character.half_width * self.s
        return Bounds(left=g.left() + m, right=g.right() + 1 - m,
                      top=g.top() + self.character.height * self.s, floor=g.bottom() + 1)

    def _tick(self) -> None:
        dt = self._elapsed.restart() / 1000.0
        b = self.brain
        cur = QCursor.pos()
        eye_y = b.y - self.character.eye_y * self.s
        b.update(dt, self._bounds(), cursor=(cur.x() - b.x, cur.y() - eye_y))
        pos = QPoint(round(b.x - self.width() / 2),
                     round(b.y - (self.base_h - FEET_FROM_BOTTOM) * self.s))
        if pos != self.pos():
            self.move(pos)
        self._render()

    def _render(self) -> None:
        dpr = self.devicePixelRatioF()
        size = QSize(round(self.width() * dpr), round(self.height() * dpr))
        if self._frame.size() != size:
            self._frame = QImage(size, QImage.Format_ARGB32_Premultiplied)
            self._frame.setDevicePixelRatio(dpr)
        self._frame.fill(Qt.transparent)
        p = QPainter(self._frame)
        p.translate(self.width() / 2, self.height() - FEET_FROM_BOTTOM * self.s)
        p.scale(self.s, self.s)
        self.character.paint(p, self.brain.pose, self.brain.facing)
        p.end()
        if self._use_mask and not self.click_through:
            self._update_mask()
        self.update()

    def _update_mask(self) -> None:
        img = self._frame
        if img.devicePixelRatio() != 1:
            img = img.scaled(self.size())
        region = QRegion(QBitmap.fromImage(img.createAlphaMask()))
        # grow by 2px: keeps antialiased edges and covers the shape lagging a frame behind
        region = region.united(region.translated(2, 0)).united(region.translated(-2, 0))
        region = region.united(region.translated(0, 2)).united(region.translated(0, -2))
        self.setMask(region if not region.isEmpty() else QRegion(0, 0, 1, 1))

    def paintEvent(self, event) -> None:
        p = QPainter(self)
        p.setCompositionMode(QPainter.CompositionMode_Source)
        p.drawImage(0, 0, self._frame)

    # ----------------------------------------------------------- input
    def mousePressEvent(self, e) -> None:
        g = e.globalPosition()
        if e.button() == Qt.LeftButton:
            self._press = g.toPoint()
            self._grab_offset = QPointF(g.x() - self.brain.x, g.y() - self.brain.y)
            self._dragging = False
        elif e.button() == Qt.RightButton and self.menu is not None:
            self.menu.popup(g.toPoint())

    def mouseMoveEvent(self, e) -> None:
        if self._press is None:
            return
        g = e.globalPosition()
        if not self._dragging and (g.toPoint() - self._press).manhattanLength() > 4:
            self._dragging = True
            self.brain.grab()
            self.setCursor(Qt.ClosedHandCursor)
        if self._dragging:
            self.brain.drag_to(g.x() - self._grab_offset.x(), g.y() - self._grab_offset.y())

    def mouseReleaseEvent(self, e) -> None:
        if e.button() != Qt.LeftButton or self._press is None:
            return
        if self._dragging:
            self.brain.release()
            self.setCursor(Qt.OpenHandCursor)
        else:
            self.brain.poke()
        self._press = None
        self._dragging = False

    # ----------------------------------------------------------- toggles
    def set_paused(self, on: bool) -> None:
        self.brain.paused = on

    def set_click_through(self, on: bool) -> None:
        if on == self.click_through:
            return
        self.click_through = on
        if on:
            self.clearMask()
        self.setWindowFlag(Qt.WindowTransparentForInput, on)
        self.show()  # changing flags hides the window

    def _reassert_topmost(self) -> None:
        """Windows: other topmost windows can end up above us; quietly climb back."""
        import ctypes
        from ctypes import wintypes
        user32 = ctypes.windll.user32
        user32.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int,
                                        ctypes.c_int, ctypes.c_int, wintypes.UINT]
        SWP_NOSIZE, SWP_NOMOVE, SWP_NOACTIVATE = 0x1, 0x2, 0x10
        user32.SetWindowPos(int(self.winId()), wintypes.HWND(-1), 0, 0, 0, 0,
                            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)

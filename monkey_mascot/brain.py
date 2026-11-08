"""Behaviour and physics for the monkey.

Pure Python (no Qt), so it can be stepped headlessly -- see tests/.
Coordinates are screen pixels; (x, y) is the point between the monkey's feet.
"""
from __future__ import annotations

import math
import random
from dataclasses import dataclass

from .pose import Pose

GRAVITY = 2600.0        # px/s²
WALK_SPEED = 75.0       # px/s
JUMP_VY = -720.0
JUMP_VX = 220.0
MAX_THROW = 1800.0
DIZZY_IMPACT = 1900.0   # landing speed (px/s) that leaves the monkey seeing stars
EAT_TIME = 5.2
BITES = (1.2, 2.3, 3.4)
LINES = ("Ook!", "Ook ook!", "Eek!", "Banana?", "Hi!")

GROUNDED = frozenset({"idle", "walk", "scratch", "eat", "crouch", "land", "dizzy"})


@dataclass
class Bounds:
    left: float     # leftmost x the feet may reach
    right: float
    top: float      # highest y the feet may reach (keeps the head on screen)
    floor: float


def _clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else hi if v > hi else v


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _ease(t: float) -> float:
    t = _clamp(t, 0.0, 1.0)
    return t * t * (3 - 2 * t)


def _bump(t: float, center: float, width: float) -> float:
    d = abs(t - center) / (width / 2)
    return math.cos(d * math.pi / 2) ** 2 if d < 1 else 0.0


class Brain:
    def __init__(self, x: float, y: float, rng: random.Random | None = None):
        self.rng = rng or random.Random()
        self.x, self.y = float(x), float(y)
        self.vx = self.vy = 0.0
        self.facing = 1
        self.paused = False
        self.clock = 0.0
        self.pose = Pose()
        self.state = "air"          # starts by dropping in
        self._t = 0.0
        self._dur = 0.0
        self._air_kind = "fall"
        self._drag_target = (self.x, self.y)
        self._drag_v = (0.0, 0.0)
        self._swing = self._swing_v = 0.0
        self._look = (0.0, 0.0)
        self._next_blink = 1.0
        self._blink_until = -1.0
        self._bubble: str | None = None
        self._bubble_until = 0.0

    # ----------------------------------------------------------- events
    def say(self, text: str, seconds: float = 1.6) -> None:
        self._bubble, self._bubble_until = text, self.clock + seconds

    def poke(self) -> None:
        self.say(self.rng.choice(LINES))
        if self.state in ("idle", "walk", "scratch", "eat"):
            self._go("crouch", 0.14)

    def grab(self) -> None:
        self._drag_target = (self.x, self.y)
        self._drag_v = (0.0, 0.0)
        self._swing = self._swing_v = 0.0
        self._go("dragged")

    def drag_to(self, x: float, y: float) -> None:
        self._drag_target = (x, y)

    def release(self) -> None:
        self.vx = _clamp(self._drag_v[0], -MAX_THROW, MAX_THROW)
        self.vy = _clamp(self._drag_v[1], -MAX_THROW, MAX_THROW)
        if abs(self.vx) > 40:
            self.facing = 1 if self.vx > 0 else -1
        self._air_kind = "fall"
        self._go("air")

    # ----------------------------------------------------------- update
    def update(self, dt: float, bounds: Bounds, cursor: tuple[float, float] | None = None) -> Pose:
        """Advance by dt seconds. cursor = mouse position relative to the eyes."""
        dt = _clamp(dt, 0.0, 0.05)
        self.clock += dt
        self._t += dt
        if self.state in GROUNDED:
            if self.y < bounds.floor - 2:   # floor moved away (screen/workarea change)
                self.vx = self.vy = 0.0
                self._air_kind = "fall"
                self._go("air")
            else:
                self.y = bounds.floor
                self.x = _clamp(self.x, bounds.left, bounds.right)
        p = Pose()
        getattr(self, "_" + self.state)(dt, bounds, p)
        self._face(dt, p, cursor)
        self.pose = p
        return p

    def _go(self, state: str, duration: float = 0.0) -> None:
        self.state, self._t, self._dur = state, 0.0, duration

    def _go_idle(self) -> None:
        self._go("idle", self.rng.uniform(1.5, 4.0))

    def _pick_next(self, b: Bounds) -> None:
        r = self.rng.random() * 100
        if r < 45:
            if b.right - self.x < 60:
                self.facing = -1
            elif self.x - b.left < 60:
                self.facing = 1
            else:
                self.facing = self.rng.choice((-1, 1))
            self._go("walk", self.rng.uniform(2.5, 7.0))
        elif r < 59:
            self._go("scratch", self.rng.uniform(1.8, 3.2))
        elif r < 69:
            self._go("eat", EAT_TIME)
        elif r < 78:
            self._go("crouch", 0.14)
        else:
            self._go_idle()

    # ----------------------------------------------------------- states
    def _breathe(self, p: Pose) -> None:
        s = math.sin(self.clock * 2.2)
        p.bob = s * 0.7
        p.tail = math.sin(self.clock * 1.4)
        p.arm_front = (12 + 2 * s, 10.0)
        p.arm_back = (-12 - 2 * s, -10.0)

    def _idle(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        if self._t >= self._dur and not self.paused:
            self._pick_next(b)

    def _walk(self, dt: float, b: Bounds, p: Pose) -> None:
        if self.paused or self._t >= self._dur:
            self._breathe(p)
            self._go_idle()
            return
        self.x += self.facing * WALK_SPEED * dt
        if self.x <= b.left:
            self.x, self.facing = b.left, 1
        elif self.x >= b.right:
            self.x, self.facing = b.right, -1
        ph = self._t * 9.0
        s, c = math.sin(ph), math.cos(ph)
        p.leg_front = (24 * s, -28 * max(0.0, c))
        p.leg_back = (-24 * s, -28 * max(0.0, -c))
        p.arm_front = (12 - 22 * s, 12.0)
        p.arm_back = (-12 + 22 * s, 12.0)
        p.lean = 4.0
        p.tail = math.sin(self.clock * 5) * 0.8

    def _scratch(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        k = _ease(self._t / 0.3) * _ease((self._dur - self._t) / 0.3)
        up = (-162.0, -28.0 + 14 * math.sin(self.clock * 28))
        p.arm_back = (_lerp(p.arm_back[0], up[0], k), _lerp(p.arm_back[1], up[1], k))
        p.head_tilt = -8 * k
        if 0.35 < self._t / self._dur < 0.8:
            p.eyes = "happy"
        if self._t >= self._dur:
            self._go_idle()

    def _eat(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        t = self._t
        k = _ease(t / 0.5) * _ease((EAT_TIME - t) / 0.5)
        bring = max(_bump(t, bt, 0.45) for bt in BITES)
        p.arm_front = (_lerp(p.arm_front[0], 30.0, k), _lerp(p.arm_front[1], 130.0, k) + 50 * bring)
        bites = sum(t > bt for bt in BITES)
        done = t >= 4.2
        p.banana = -1 if done else bites
        p.head_tilt = 4 * bring
        if bring > 0.35:
            p.mouth = "o"
        elif done:
            p.mouth, p.eyes = "big", "happy"
        elif bites:
            p.mouth = "chew" if math.sin(self.clock * 16) > 0 else "smile"
        if t >= EAT_TIME:
            self._go_idle()

    def _crouch(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        k = _ease(self._t / self._dur)
        p.squash = 1 - 0.14 * k
        p.leg_front = (28 * k, -56 * k)
        p.leg_back = (-28 * k, 56 * k)
        p.arm_front = (12 + 25 * k, 10.0)
        p.arm_back = (-12 - 25 * k, -10.0)
        if self._t >= self._dur:
            self.vy = JUMP_VY * self.rng.uniform(0.85, 1.1)
            self.vx = 0.0 if self.paused else self.facing * JUMP_VX * self.rng.uniform(0.3, 1.0)
            self._air_kind = "jump"
            self._go("air")

    def _air(self, dt: float, b: Bounds, p: Pose) -> None:
        self.vy += GRAVITY * dt
        self.x += self.vx * dt
        self.y += self.vy * dt
        if self.x < b.left:
            self.x, self.vx, self.facing = b.left, abs(self.vx) * 0.5, 1
        elif self.x > b.right:
            self.x, self.vx, self.facing = b.right, -abs(self.vx) * 0.5, -1
        if self.y < b.top and self.vy < 0:
            self.y, self.vy = b.top, -self.vy * 0.3

        c = self.clock
        p.grounded = False
        p.lean = _clamp(self.vx * self.facing * 0.012, -12, 12)
        p.tail = math.sin(c * 8)
        if self._air_kind == "jump":
            p.squash = 1.06 if self.vy < 0 else 1.0
            p.arm_front, p.arm_back = (150.0, 15.0), (-150.0, -15.0)
            p.leg_front, p.leg_back = (35.0, -70.0), (-35.0, 70.0)
            p.mouth = "big"
        else:
            w = math.sin(c * 22)
            p.arm_front = (135 + 30 * w, 15.0)
            p.arm_back = (-135 + 30 * w, -15.0)
            p.leg_front = (20 * math.sin(c * 16), -25.0)
            p.leg_back = (-20 * math.sin(c * 16 + 1.3), 25.0)
            p.eyes, p.mouth = "wide", "o"

        if self.vy > 0 and self.y >= b.floor:
            impact = self.vy
            self.y = b.floor
            self.vx = self.vy = 0.0
            if impact > DIZZY_IMPACT:
                self._go("dizzy", 2.6)
            else:
                self._go("land", 0.22)

    def _land(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        bend = 1 - _ease(self._t / self._dur)
        p.squash = 1 - 0.26 * bend
        p.leg_front = (30 * bend, -60 * bend)
        p.leg_back = (-30 * bend, 60 * bend)
        p.arm_front = (12 + 50 * bend, 10.0)
        p.arm_back = (-12 - 50 * bend, -10.0)
        if self._t >= self._dur:
            self._go_idle()

    def _dizzy(self, dt: float, b: Bounds, p: Pose) -> None:
        self._breathe(p)
        c = self.clock
        p.eyes, p.mouth = "dizzy", "o"
        p.stars = c * 4.5
        p.lean = 5 * math.sin(c * 3.2)
        p.head_tilt = 7 * math.sin(c * 3.2 + 1.2)
        if self._t >= self._dur:
            self._go_idle()

    def _dragged(self, dt: float, b: Bounds, p: Pose) -> None:
        tx, ty = self._drag_target
        if dt > 0:
            a = _clamp(dt * 14, 0.0, 1.0)
            self._drag_v = (_lerp(self._drag_v[0], (tx - self.x) / dt, a),
                            _lerp(self._drag_v[1], (ty - self.y) / dt, a))
        self.x, self.y = tx, ty
        # pendulum: the body trails behind the direction of motion
        target = _clamp(self._drag_v[0] * 0.03, -40, 40)
        self._swing_v += ((target - self._swing) * 90 - self._swing_v * 9) * dt
        self._swing = _clamp(self._swing + self._swing_v * dt, -45, 45)

        c = self.clock
        p.grounded = False
        p.swing = self._swing * self.facing   # painter is mirrored when facing left
        p.arm_front = (28 + 8 * math.sin(c * 5), 12.0)
        p.arm_back = (-28 - 8 * math.sin(c * 5 + 0.6), -12.0)
        p.leg_front = (12 * math.sin(c * 7), -18.0)
        p.leg_back = (-12 * math.sin(c * 7 + 1.6), 18.0)
        p.eyes, p.mouth = "wide", "o"
        p.tail = math.sin(c * 3)

    # ----------------------------------------------------------- face
    def _face(self, dt: float, p: Pose, cursor: tuple[float, float] | None) -> None:
        target = (0.0, 0.0)
        if cursor is not None and p.eyes in ("open", "wide"):
            dx, dy = cursor
            d = math.hypot(dx, dy) or 1.0
            reach = min(1.0, d / 80)
            target = (dx / d * reach, dy / d * reach)
        a = _clamp(dt * 10, 0.0, 1.0)
        self._look = (_lerp(self._look[0], target[0], a), _lerp(self._look[1], target[1], a))
        p.look = self._look

        if self.clock >= self._next_blink:
            self._blink_until = self.clock + 0.13
            self._next_blink = self.clock + self.rng.uniform(2.0, 6.0)
        if self.clock < self._blink_until and p.eyes == "open":
            p.eyes = "closed"
        if self._bubble and self.clock < self._bubble_until:
            p.bubble = self._bubble

"""Shared mascot engine: base pose, physics and the behaviour state machine.

Pure Python (no Qt), so behaviour can be simulated headlessly -- see tests/.
Coordinates are screen pixels; (x, y) is the point between the mascot's feet.

A character subclasses Brain, supplies a Pose dataclass, the pose_* hooks for
the shared states (idle, walk, crouch -> air, land, dizzy, dragged) and any
extra states of its own as methods named _<state>(dt, bounds, pose).
"""
from __future__ import annotations

import math
import random
from dataclasses import dataclass
from typing import Callable, ClassVar

GRAVITY = 2600.0        # px/s²
MAX_THROW = 1800.0      # px/s


@dataclass
class Bounds:
    left: float     # leftmost x the feet may reach
    right: float
    top: float      # highest y the feet may reach (keeps the head on screen)
    floor: float


@dataclass
class BasePose:
    bob: float = 0.0            # vertical body offset, px (negative = up)
    squash: float = 1.0         # <1 squashed, >1 stretched (pivot: feet)
    lean: float = 0.0           # whole-body tilt, degrees, positive = forward
    swing: float = 0.0          # pendulum swing around the grab point (when dragged)
    eyes: str = "open"          # open | wide | closed | happy | dizzy
    look: tuple[float, float] = (0.0, 0.0)  # pupil direction (screen space), -1..1
    stars: float | None = None  # dizzy stars orbit angle (radians)
    bubble: str | None = None   # speech bubble text
    grounded: bool = True       # keep the lowest foot planted on the floor


@dataclass(frozen=True)
class Character:
    name: str
    brain: type                 # Brain subclass
    paint: Callable             # paint(painter, pose, facing, head_only=False)
    half_width: float           # feet -> furthest pixel sideways (screen-edge collisions)
    height: float               # feet -> top of head
    eye_y: float                # feet -> eyes (the gaze follows the cursor from here)
    icon_box: tuple[float, float, float]  # (cx, cy, extent) framed by the tray icon
    size: tuple[int, int] = (170, 190)    # window size at scale 1
    previews: tuple = ()        # (label, state, duration, at) extra scenes for tools/preview.py


def clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else hi if v > hi else v


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def lerp2(a: tuple[float, float], b: tuple[float, float], t: float) -> tuple[float, float]:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def ease(t: float) -> float:
    t = clamp(t, 0.0, 1.0)
    return t * t * (3 - 2 * t)


def bump(t: float, center: float, width: float) -> float:
    """Smooth 0 -> 1 -> 0 pulse of the given width around center."""
    d = abs(t - center) / (width / 2)
    return math.cos(d * math.pi / 2) ** 2 if d < 1 else 0.0


class Brain:
    pose_cls: ClassVar[type] = BasePose
    WALK_SPEED: ClassVar[float] = 75.0
    JUMP_VY: ClassVar[float] = -720.0
    JUMP_VX: ClassVar[float] = 220.0
    DIZZY_IMPACT: ClassVar[float] = 1900.0   # landing speed that leaves it seeing stars
    LINES: ClassVar[tuple[str, ...]] = ("Hi!",)
    # (weight, state, (min_seconds, max_seconds)) picked whenever idle time runs out
    CHOICES: ClassVar[tuple] = (
        (45, "walk", (2.5, 7.0)),
        (10, "crouch", (0.14, 0.14)),
        (25, "idle", (1.5, 4.0)),
    )

    def __init__(self, x: float, y: float, rng: random.Random | None = None):
        self.rng = rng or random.Random()
        self.x, self.y = float(x), float(y)
        self.vx = self.vy = 0.0
        self.facing = 1
        self.paused = False
        self.clock = 0.0            # seconds since birth
        self.t = 0.0                # seconds in the current state
        self.duration = 0.0         # planned length of the current state
        self.state = "air"          # starts by dropping in
        self.cursor: tuple[float, float] | None = None  # mouse, relative to the eyes
        self.pose = self.pose_cls()
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
    @property
    def airborne(self) -> bool:
        return self.state in ("air", "dragged")

    @property
    def entered(self) -> bool:
        """True on the first update of the current state."""
        return self.t <= self._dt + 1e-9

    def say(self, text: str, seconds: float = 1.6) -> None:
        self._bubble, self._bubble_until = text, self.clock + seconds

    def poke(self) -> None:
        self.say(self.rng.choice(self.LINES))
        if not self.airborne and self.state not in ("crouch", "land", "dizzy"):
            self.go("crouch", 0.14)

    def grab(self) -> None:
        self._drag_target = (self.x, self.y)
        self._drag_v = (0.0, 0.0)
        self._swing = self._swing_v = 0.0
        self.go("dragged")

    def drag_to(self, x: float, y: float) -> None:
        self._drag_target = (x, y)

    def release(self) -> None:
        self.vx = clamp(self._drag_v[0], -MAX_THROW, MAX_THROW)
        self.vy = clamp(self._drag_v[1], -MAX_THROW, MAX_THROW)
        if abs(self.vx) > 40:
            self.facing = 1 if self.vx > 0 else -1
        self._air_kind = "fall"
        self.go("air")

    # ----------------------------------------------------------- update
    _dt = 0.0

    def update(self, dt: float, bounds: Bounds, cursor: tuple[float, float] | None = None):
        """Advance by dt seconds. cursor = mouse position relative to the eyes."""
        dt = clamp(dt, 0.0, 0.05)
        self._dt = dt
        self.clock += dt
        self.t += dt
        self.cursor = cursor
        if not self.airborne:
            if self.y < bounds.floor - 2:   # floor moved away (screen/workarea change)
                self.vx = self.vy = 0.0
                self._air_kind = "fall"
                self.go("air")
            else:
                self.y = bounds.floor
                self.x = clamp(self.x, bounds.left, bounds.right)
        p = self.pose_cls()
        getattr(self, "_" + self.state)(dt, bounds, p)
        self._face(dt, p)
        self.pose = p
        return p

    def go(self, state: str, duration: float = 0.0) -> None:
        self.state, self.t, self.duration = state, 0.0, duration

    def go_idle(self) -> None:
        self.go("idle", self.rng.uniform(1.5, 4.0))

    @property
    def done(self) -> bool:
        return self.t >= self.duration

    def step_x(self, b: Bounds, dx: float) -> None:
        """Walk horizontally, turning around at the screen edges."""
        self.x += dx
        if self.x <= b.left:
            self.x, self.facing = b.left, 1
        elif self.x >= b.right:
            self.x, self.facing = b.right, -1

    def _pick_next(self, b: Bounds) -> None:
        r = self.rng.random() * sum(w for w, _, _ in self.CHOICES)
        for weight, state, (lo, hi) in self.CHOICES:
            r -= weight
            if r < 0:
                break
        if state == "walk":
            if b.right - self.x < 60:
                self.facing = -1
            elif self.x - b.left < 60:
                self.facing = 1
            else:
                self.facing = self.rng.choice((-1, 1))
        self.go(state, self.rng.uniform(lo, hi))

    # ----------------------------------------------------------- pose hooks
    def pose_rest(self, p) -> None:
        p.bob = math.sin(self.clock * 2.2) * 0.7

    def pose_walk(self, p, t: float) -> None:
        self.pose_rest(p)

    def pose_crouch(self, p, k: float) -> None:
        self.pose_rest(p)

    def pose_jump(self, p) -> None:
        pass

    def pose_fall(self, p) -> None:
        pass

    def pose_land(self, p, bend: float) -> None:
        self.pose_rest(p)

    def pose_dizzy(self, p) -> None:
        pass

    def pose_dragged(self, p) -> None:
        pass

    # ----------------------------------------------------------- shared states
    def _idle(self, dt: float, b: Bounds, p) -> None:
        self.pose_rest(p)
        if self.done and not self.paused:
            self._pick_next(b)

    def _walk(self, dt: float, b: Bounds, p) -> None:
        if self.paused or self.done:
            self.pose_rest(p)
            self.go_idle()
            return
        self.step_x(b, self.facing * self.WALK_SPEED * dt)
        self.pose_walk(p, self.t)

    def _crouch(self, dt: float, b: Bounds, p) -> None:
        k = ease(self.t / self.duration)
        self.pose_crouch(p, k)
        p.squash = min(p.squash, 1 - 0.14 * k)
        if self.done:
            self.vy = self.JUMP_VY * self.rng.uniform(0.85, 1.1)
            self.vx = 0.0 if self.paused else self.facing * self.JUMP_VX * self.rng.uniform(0.3, 1.0)
            self._air_kind = "jump"
            self.go("air")

    def _air(self, dt: float, b: Bounds, p) -> None:
        self.vy += GRAVITY * dt
        self.x += self.vx * dt
        self.y += self.vy * dt
        if self.x < b.left:
            self.x, self.vx, self.facing = b.left, abs(self.vx) * 0.5, 1
        elif self.x > b.right:
            self.x, self.vx, self.facing = b.right, -abs(self.vx) * 0.5, -1
        if self.y < b.top and self.vy < 0:
            self.y, self.vy = b.top, -self.vy * 0.3

        p.grounded = False
        p.lean = clamp(self.vx * self.facing * 0.012, -12, 12)
        if self._air_kind == "jump":
            p.squash = 1.06 if self.vy < 0 else 1.0
            self.pose_jump(p)
        else:
            p.eyes = "wide"
            self.pose_fall(p)

        if self.vy > 0 and self.y >= b.floor:
            impact = self.vy
            self.y = b.floor
            self.vx = self.vy = 0.0
            if impact > self.DIZZY_IMPACT:
                self.go("dizzy", 2.6)
            else:
                self.go("land", 0.22)

    def _land(self, dt: float, b: Bounds, p) -> None:
        bend = 1 - ease(self.t / self.duration)
        self.pose_land(p, bend)
        p.squash = 1 - 0.26 * bend
        if self.done:
            self.go_idle()

    def _dizzy(self, dt: float, b: Bounds, p) -> None:
        self.pose_rest(p)
        p.eyes = "dizzy"
        p.stars = self.clock * 4.5
        p.lean = 5 * math.sin(self.clock * 3.2)
        self.pose_dizzy(p)
        if self.done:
            self.go_idle()

    def _dragged(self, dt: float, b: Bounds, p) -> None:
        tx, ty = self._drag_target
        if dt > 0:
            a = clamp(dt * 14, 0.0, 1.0)
            self._drag_v = (lerp(self._drag_v[0], (tx - self.x) / dt, a),
                            lerp(self._drag_v[1], (ty - self.y) / dt, a))
        self.x, self.y = tx, ty
        # pendulum: the body trails behind the direction of motion
        target = clamp(self._drag_v[0] * 0.03, -40, 40)
        self._swing_v += ((target - self._swing) * 90 - self._swing_v * 9) * dt
        self._swing = clamp(self._swing + self._swing_v * dt, -45, 45)
        p.grounded = False
        p.swing = self._swing * self.facing   # painters mirror when facing left
        p.eyes = "wide"
        self.pose_dragged(p)

    # ----------------------------------------------------------- face
    def _face(self, dt: float, p) -> None:
        target = (0.0, 0.0)
        if self.cursor is not None and p.eyes in ("open", "wide"):
            dx, dy = self.cursor
            d = math.hypot(dx, dy) or 1.0
            reach = min(1.0, d / 80)
            target = (dx / d * reach, dy / d * reach)
        self._look = lerp2(self._look, target, clamp(dt * 10, 0.0, 1.0))
        p.look = self._look

        if self.clock >= self._next_blink:
            self._blink_until = self.clock + 0.13
            self._next_blink = self.clock + self.rng.uniform(2.0, 6.0)
        if self.clock < self._blink_until and p.eyes == "open":
            p.eyes = "closed"
        if self._bubble and self.clock < self._bubble_until:
            p.bubble = self._bubble

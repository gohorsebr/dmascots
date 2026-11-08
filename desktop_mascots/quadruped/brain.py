"""Shared four-legged rig: pose, gaits and common quadruped behaviours (Qt-free).

A QuadPose doesn't store joint angles. It says where the shoulders and hips
are and where each foot should be; the painter solves the knees with
two-bone IK. So "sit" is just "drop the hips and tuck the hind feet forward",
and the legs fold correctly for every species and size.
"""
from __future__ import annotations

import math
from dataclasses import dataclass
from typing import ClassVar

from ..engine import BasePose, Bounds, Brain, clamp, ease, lerp, lerp2

FN, FF, HN, HF = range(4)          # foot order: front near/far, hind near/far
STAND = ((0.0, 0.0, False),) * 4   # (dx from the joint's rest x, lift, hang from the joint)


@dataclass(frozen=True)
class Rig:
    shoulder: tuple[float, float]      # rest joint at the top of the front legs (feet at y=0)
    hip: tuple[float, float]
    front_leg: tuple[float, float]     # (upper, lower) bone lengths
    hind_leg: tuple[float, float]
    head: tuple[float, float]          # rest head centre
    head_angle: float = 0.0            # rest head angle, positive tips the nose down
    neck_base: tuple[float, float] = (2.0, -6.0)   # from the shoulder
    tail_root: tuple[float, float] = (-6.0, -4.0)  # from the hip
    foot_y: float = 0.0                # where the foot target sits when planted (paw thickness)

    @property
    def height(self) -> float:
        return -min(self.shoulder[1], self.hip[1])


@dataclass
class QuadPose(BasePose):
    shoulder: tuple[float, float] = (0.0, 0.0)  # offsets from the rig's rest joints
    hip: tuple[float, float] = (0.0, 0.0)
    feet: tuple = STAND
    head: tuple[float, float] = (0.0, 0.0)      # offset from the rig's rest head
    head_angle: float = 0.0                     # offset from the rig's rest angle
    mouth: float = 0.0          # 0 closed .. 1 wide open
    tongue: bool = False
    ears: float = 0.0           # -1 pinned back .. 0 relaxed .. 1 perked
    tail: float = 0.0           # angle offset, degrees, positive = up
    tail_wag: float = 0.0       # sideways swish, degrees
    tail_curl: float = 0.0      # 0 straight .. 1 hooked up at the tip
    zzz: float | None = None    # asleep: phase of the floating z's
    action: float = 0.0         # species-specific extra (e.g. a lifted paw for grooming)


class QuadBrain(Brain):
    pose_cls = QuadPose
    rig: ClassVar[Rig]
    STRIDE: ClassVar[float] = 8.0       # foot swing amplitude when walking, px
    LIFT: ClassVar[float] = 4.0         # foot lift when walking, px
    RUN_SPEED: ClassVar[float] = 200.0
    RUN_GAIT: ClassVar[str] = "gallop"

    # ----------------------------------------------------------- building blocks
    def breathe(self, p: QuadPose) -> None:
        s = math.sin(self.clock * 2.0)
        p.shoulder = (0.0, -0.5 * s)
        p.hip = (0.0, -0.3 * s)
        p.tail_wag = 5 * math.sin(self.clock * 1.4)

    def gait(self, p: QuadPose, phase: float, stride: float, lift: float, kind: str = "walk") -> None:
        offsets = {"walk": (0.0, math.pi, math.pi * 1.5, math.pi * 0.5),
                   "trot": (0.0, math.pi, math.pi, 0.0),
                   "gallop": (0.0, 0.7, math.pi, math.pi + 0.7)}[kind]
        p.feet = tuple((stride * math.sin(phase + o), lift * max(0.0, math.cos(phase + o)), False)
                       for o in offsets)
        if kind == "gallop":   # rocking-horse seesaw
            p.shoulder = (0.0, -lift * 0.5 * math.sin(phase))
            p.hip = (0.0, lift * 0.5 * math.sin(phase))
            p.lean = -3 * math.sin(phase)
        else:
            bob = -lift * 0.3 * abs(math.cos(phase * (2 if kind == "walk" else 1)))
            p.shoulder = (0.0, bob)
            p.hip = (0.0, bob)

    def move(self, p: QuadPose, b: Bounds, dt: float, speed: float, kind: str,
             stride: float | None = None, lift: float | None = None) -> None:
        """Walk/run at speed with a gait whose feet don't slide on the floor."""
        stride = stride or self.STRIDE * (1.6 if kind != "walk" else 1.0)
        lift = lift or self.LIFT * (1.8 if kind != "walk" else 1.0)
        self.step_x(b, self.facing * speed * dt)
        self._phase = getattr(self, "_phase", 0.0) + speed / stride * dt
        self.gait(p, self._phase, stride, lift, kind)

    def sit(self, p: QuadPose, k: float) -> None:
        r = self.rig
        drop = 0.62 * -r.hip[1]
        p.hip = lerp2(p.hip, (6.0, drop), k)
        p.shoulder = lerp2(p.shoulder, (0.0, -2.0), k)
        p.feet = (p.feet[FN], p.feet[FF], (8 * k, 0.0, False), (6 * k, 0.0, False))
        p.head = lerp2(p.head, (-2.0, -3.0), k)
        p.tail = lerp(p.tail, -25.0, k)

    def lie(self, p: QuadPose, k: float) -> None:
        r = self.rig
        p.shoulder = lerp2(p.shoulder, (3.0, 0.68 * -r.shoulder[1]), k)
        p.hip = lerp2(p.hip, (4.0, 0.66 * -r.hip[1]), k)
        reach = sum(r.front_leg) * 0.75
        p.feet = ((reach * k, 0.0, False), (reach * 0.8 * k, 0.0, False),
                  (10 * k, 0.0, False), (8 * k, 0.0, False))
        p.head = lerp2(p.head, (3.0, 0.55 * -r.shoulder[1]), k)
        p.tail = lerp(p.tail, -30.0, k)

    def bow(self, p: QuadPose, k: float) -> None:
        """Play bow / stretch: chest down, rear up."""
        r = self.rig
        p.shoulder = lerp2(p.shoulder, (2.0, 0.5 * -r.shoulder[1]), k)
        p.hip = lerp2(p.hip, (0.0, -2.0), k)
        reach = sum(r.front_leg) * 0.6
        p.feet = ((reach * k, 0.0, False), (reach * 0.85 * k, 0.0, False), p.feet[HN], p.feet[HF])
        p.head = lerp2(p.head, (6.0, 0.45 * -r.shoulder[1]), k)
        p.tail = lerp(p.tail, 25.0, k)

    def hang(self, p: QuadPose, kick: float = 0.0, tuck: float = 0.0) -> None:
        """Feet dangle from the joints (airborne, dragged)."""
        c = self.clock
        p.feet = tuple((kick * math.sin(c * 11 + i * 1.7) + (4 if i < 2 else -4) * tuck,
                        tuck * 8 + abs(kick) * 0.3 * (1 + math.sin(c * 11 + i * 1.7)), True)
                       for i in range(4))

    def head_to_ground(self, p: QuadPose, k: float, forward: float = 8.0, nose_down: float = 45.0) -> None:
        r = self.rig
        p.head = lerp2(p.head, (forward, -r.head[1] - 9.0), k)
        p.head_angle = lerp(p.head_angle, nose_down, k)

    def stalk(self, p: QuadPose, b: Bounds, dt: float, speed: float) -> float:
        """Creep low toward the cursor; returns the remaining horizontal distance."""
        dx = self.cursor[0] if self.cursor else 0.0
        if abs(dx) > 4:
            self.facing = 1 if dx > 0 else -1
        r = self.rig
        self.move(p, b, dt, speed if abs(dx) > 8 else 0.0, "walk", lift=self.LIFT * 0.6)
        p.shoulder = (p.shoulder[0], p.shoulder[1] + 0.35 * -r.shoulder[1])
        p.hip = (p.hip[0], p.hip[1] + 0.3 * -r.hip[1])
        p.head = (4.0, 0.3 * -r.shoulder[1])
        p.head_angle = -5
        p.ears = 1.0
        p.eyes = "wide"
        p.tail = -10
        return abs(dx)

    # ----------------------------------------------------------- shared-state poses
    def pose_rest(self, p: QuadPose) -> None:
        self.breathe(p)

    def pose_walk(self, p: QuadPose, t: float) -> None:
        self._phase = t * self.WALK_SPEED / self.STRIDE
        self.gait(p, self._phase, self.STRIDE, self.LIFT, "walk")
        p.tail_wag = 8 * math.sin(self._phase)

    def pose_crouch(self, p: QuadPose, k: float) -> None:
        self.breathe(p)
        p.shoulder = (0.0, 5 * k)
        p.hip = (0.0, 5 * k)
        p.ears = k

    def pose_jump(self, p: QuadPose) -> None:
        self.hang(p, tuck=1.0)
        p.tail = 15
        p.ears = 1.0

    def pose_fall(self, p: QuadPose) -> None:
        self.hang(p, kick=6.0)
        p.mouth = 0.6
        p.ears = -1.0
        p.tail = 20 * math.sin(self.clock * 9)

    def pose_land(self, p: QuadPose, bend: float) -> None:
        self.breathe(p)
        p.shoulder = (0.0, 7 * bend)
        p.hip = (0.0, 7 * bend)
        p.feet = ((4 * bend, 0.0, False), (2 * bend, 0.0, False), (-4 * bend, 0.0, False), (-2 * bend, 0.0, False))

    def pose_dizzy(self, p: QuadPose) -> None:
        c = self.clock
        p.head = (2 * math.cos(c * 4), 2 * math.sin(c * 4))
        p.head_angle = 10 * math.sin(c * 4)
        p.ears = -0.5
        p.tail = -20

    def pose_dragged(self, p: QuadPose) -> None:
        self.hang(p, kick=3.0)
        p.ears = -1.0
        p.tail = -35
        p.head_angle = -10

    # ----------------------------------------------------------- shared states
    def _run(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """Zoomies: a fast run, bouncing off the screen edges."""
        if self.paused or self.done:
            self.pose_rest(p)
            self.go_idle()
            return
        ramp = ease(self.t / 0.4) * ease((self.duration - self.t) / 0.4)
        self.move(p, b, dt, self.RUN_SPEED * max(0.25, ramp), self.RUN_GAIT)
        p.ears = -0.6
        p.tail = 10
        p.lean = p.lean + 3

    def _sit(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        self.sit(p, ease(self.t / 0.4) * ease((self.duration - self.t) / 0.4))
        if self.done:
            self.go_idle()

    def _nap(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        k = ease(self.t / 0.6) * ease((self.duration - self.t) / 0.5)
        self.lie(p, k)
        if k > 0.95:
            p.eyes = "closed"
            p.zzz = self.clock
            p.ears = -0.3
        if self.done:
            self.go_idle()

    def _sniff(self, dt: float, b: Bounds, p: QuadPose) -> None:
        if self.paused or self.done:
            self.pose_rest(p)
            self.go_idle()
            return
        k = ease(self.t / 0.35) * ease((self.duration - self.t) / 0.35)
        self.move(p, b, dt, self.WALK_SPEED * 0.35 * k, "walk")   # nose to the floor, pottering
        self.head_to_ground(p, k)
        p.head = (p.head[0] + 1.5 * math.sin(self.clock * 25) * k, p.head[1])
        p.tail_wag = 10 * math.sin(self.clock * 6)

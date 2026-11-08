"""Horse: walks, trots, gallops, grazes, whinnies, rears, swishes its tail."""
from __future__ import annotations

import math

from ...engine import Bounds, bump, ease, keyframes
from ...quadruped.brain import FF, FN, HF, HN, QuadBrain, QuadPose, Rig

HORSE_RIG = Rig(shoulder=(24.0, -62.0), hip=(-24.0, -62.0), front_leg=(30.0, 33.0), hind_leg=(32.0, 32.0),
                head=(60.0, -100.0), head_angle=40.0, neck_base=(4.0, -8.0), tail_root=(-14.0, -8.0))
NEIGH_TIME = 1.6
REAR_TIME = 2.0


class HorseBrain(QuadBrain):
    rig = HORSE_RIG
    STEAL_SPEED = 300.0
    STEAL_LINES = ("Neigh!", "Giddy-up!")
    RETURN_LINES = ("*snort*", "Hrrm. Here.")
    WALK_SPEED = 55.0
    STRIDE = 12.0
    LIFT = 7.0
    TROT_SPEED = 140.0
    RUN_SPEED = 340.0
    JUMP_VY = -520.0
    DIZZY_IMPACT = 1700.0
    LINES = ("Neigh!", "Hrrrm.", "*snort*", "Nicker…")
    CHOICES = (
        (24, "walk", (3.0, 7.0)),
        (10, "trot", (2.0, 4.0)),
        (6, "run", (1.5, 3.0)),
        (18, "graze", (3.0, 7.0)),
        (8, "neigh", (NEIGH_TIME, NEIGH_TIME)),
        (5, "rear", (REAR_TIME, REAR_TIME)),
        (10, "swish", (1.5, 3.0)),
        (2, "crouch", (0.16, 0.16)),
        (12, "idle", (1.5, 4.0)),
    )

    def poke(self) -> None:
        if not self.airborne and self.state not in ("crouch", "land", "dizzy", "rear"):
            self.go("rear" if self.rng.random() < 0.3 else "neigh",
                    REAR_TIME if self.rng.random() < 0.3 else NEIGH_TIME)
        else:
            self.say(self.rng.choice(self.LINES))

    def pose_rest(self, p: QuadPose) -> None:
        self.breathe(p)
        c = self.clock
        p.tail_wag = 10 * math.sin(c * 1.1)
        p.head = (0.0, 1.5 * math.sin(c * 0.8))
        p.head_angle = 3 * math.sin(c * 0.6)

    def pose_walk(self, p: QuadPose, t: float) -> None:
        super().pose_walk(p, t)
        p.head = (0.0, 2.5 * math.sin(self._phase * 2))   # head nods with each step
        p.tail_wag = 8 * math.sin(self._phase)

    # ----------------------------------------------------------- horse states
    def _trot(self, dt: float, b: Bounds, p: QuadPose) -> None:
        if self.paused or self.done:
            self.pose_rest(p)
            self.go_idle()
            return
        self.move(p, b, dt, self.TROT_SPEED, "trot")
        p.head = (2.0, -2.0)
        p.head_angle = -6
        p.ears = 1.0
        p.tail = 10

    def _run(self, dt: float, b: Bounds, p: QuadPose) -> None:
        super()._run(dt, b, p)
        p.head = (8.0, 6.0)          # neck stretched out at the gallop
        p.head_angle = 8
        p.tail = 25

    def _graze(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.8) * ease((self.duration - self.t) / 0.8)
        self.head_to_ground(p, k, forward=10.0, nose_down=30.0)
        p.head = (p.head[0], p.head[1] - 20 * k)          # muzzle (not the skull) in the grass
        p.mouth = 0.2 * max(0.0, math.sin(self.clock * 7)) * k
        p.feet = ((6 * k, 0.0, False), (2 * k, 0.0, False), p.feet[HN], p.feet[HF])
        if self.done:
            self.go_idle()

    def _neigh(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.pose_rest(p)
        if self.entered:
            self.say(self.rng.choice(("Neeeigh!", "Neigh-hey-hey!", "Hrrrrm!")), 1.6)
        k = ease(self.t / 0.25) * ease((self.duration - self.t) / 0.3)
        p.head = (-6 * k, -10 * k)
        p.head_angle = -25 * k
        p.mouth = (0.55 + 0.3 * math.sin(self.clock * 34)) * k if 0.2 < self.t < 1.25 else 0.0
        p.ears, p.tail = 1.0, 20 * k
        if self.done:
            self.go_idle()

    def _rear(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """Up on the hind legs, front hooves paddling the air."""
        self.breathe(p)
        if self.entered:
            self.say("NEIGH!", 1.4)
        t = self.t
        (k,) = keyframes(t, ((0.0, 0.0), (0.45, 1.0), (1.4, 1.0), (REAR_TIME, 0.0)))
        p.shoulder = (-20 * k, -34 * k)
        p.hip = (6 * k, 4 * k)
        p.head = (-24 * k, -38 * k)
        p.head_angle = -30 * k
        paddle = math.sin(self.clock * 9)
        if k > 0.15:
            p.feet = ((6 + 6 * paddle, 18 * k, True), (8 - 6 * paddle, 16 * k, True),
                      (14 * k, 0.0, False), (12 * k, 0.0, False))
        p.mouth = 0.6 * bump(t, 0.7, 0.8)
        p.tail = 30 * k
        p.ears = -0.3
        if self.done:
            self.go_idle()

    def _swish(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.pose_rest(p)
        c = self.clock
        p.tail_wag = 45 * math.sin(c * 7)
        toss = bump(self.t % 1.5, 0.5, 0.5)
        p.head_angle = -18 * toss
        p.head = (0.0, -4 * toss)
        if self.done:
            self.go_idle()

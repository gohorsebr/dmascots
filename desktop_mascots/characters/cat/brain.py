"""Cat: walks tail-up, sits, grooms, loafs, stretches, and stalks + pounces on the cursor."""
from __future__ import annotations

import math

from ...engine import Bounds, bump, clamp, ease
from ...quadruped.brain import FN, QuadBrain, QuadPose, Rig

CAT_RIG = Rig(shoulder=(11.0, -21.0), hip=(-11.0, -22.0), front_leg=(10.5, 11.0), hind_leg=(11.5, 11.5),
              head=(21.0, -33.0), neck_base=(2.0, -5.0), tail_root=(-6.0, -3.0), foot_y=-1.6)
MEOW_TIME = 0.9


class CatBrain(QuadBrain):
    rig = CAT_RIG
    STEAL_SPEED = 250.0
    STEAL_LINES = ("Mine.", "Mrrp!", "*knocks it over*")
    RETURN_LINES = ("Bored now.", "…fine.")
    WALK_SPEED = 45.0
    STRIDE = 6.0
    LIFT = 3.5
    RUN_SPEED = 240.0
    JUMP_VY = -560.0
    LINES = ("Meow.", "Mrrp?", "Mew!", "Prrr…")
    CHOICES = (
        (22, "walk", (2.0, 5.0)),
        (14, "sit", (3.0, 6.0)),
        (10, "groom", (3.0, 5.0)),
        (10, "nap", (6.0, 12.0)),
        (6, "stretch", (2.0, 2.0)),
        (9, "pounce", (6.0, 6.0)),
        (5, "run", (1.2, 2.2)),
        (4, "meow", (MEOW_TIME, MEOW_TIME)),
        (4, "crouch", (0.14, 0.14)),
        (12, "idle", (1.5, 4.0)),
    )

    def poke(self) -> None:
        if self.state == "nap":
            self.say("…?")
            self.go_idle()
        elif not self.airborne and self.state not in ("crouch", "land", "dizzy", "pounce"):
            self.go("meow", MEOW_TIME)
        else:
            self.say(self.rng.choice(self.LINES))

    def pose_rest(self, p: QuadPose) -> None:
        self.breathe(p)
        c = self.clock
        p.tail = 15 + 10 * math.sin(c * 0.9)
        p.tail_curl = 0.5 + 0.3 * math.sin(c * 1.3)
        p.tail_wag = 6 * math.sin(c * 1.7)
        if math.sin(c * 0.7) > 0.97:      # the occasional ear flick
            p.ears = -0.8

    def pose_walk(self, p: QuadPose, t: float) -> None:
        super().pose_walk(p, t)
        p.tail, p.tail_curl = 60.0, 0.7    # tail up: a happy cat
        p.tail_wag = 4 * math.sin(t * 3)

    def pose_dragged(self, p: QuadPose) -> None:
        super().pose_dragged(p)
        p.tail_curl = 0.2

    # ----------------------------------------------------------- cat states
    def _sit(self, dt: float, b: Bounds, p: QuadPose) -> None:
        super()._sit(dt, b, p)
        k = ease(self.t / 0.4)
        p.tail, p.tail_curl = -8 * k + p.tail * (1 - k), 1.0 * k    # tail along the floor, tip hooked up
        if 0.4 < (self.t % 3.0) < 0.75:
            p.eyes = "happy"            # slow blink: cat for "I like you"

    def _nap(self, dt: float, b: Bounds, p: QuadPose) -> None:
        super()._nap(dt, b, p)
        p.tail, p.tail_curl = -5.0, 1.0

    def _groom(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """Sit, lick a paw, wipe the face with it."""
        self.pose_rest(p)
        k = ease(self.t / 0.4) * ease((self.duration - self.t) / 0.4)
        self.sit(p, k)
        p.tail, p.tail_curl = -8 * k, k
        lick = math.sin(self.clock * 9)
        wipe = (self.t % 2.0) > 1.2      # alternate: lick the paw, then wipe the face
        if k > 0.5:
            paw = (9.0, 21.0 + (3 * lick if not wipe else 6 + 4 * lick), True)
            p.feet = (paw,) + p.feet[1:]
        p.head = (p.head[0] - 4 * k, p.head[1] + (4 if not wipe else 1) * k)
        p.head_angle = (30 if not wipe else 10 + 8 * lick) * k
        p.mouth = 0.3 * max(0.0, lick) * k if not wipe else 0.0
        p.tongue = not wipe and k > 0.5
        p.eyes = "closed" if k > 0.6 else "open"
        if self.done:
            self.go_idle()

    def _stretch(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        k = ease(self.t / 0.6) * ease((self.duration - self.t) / 0.5)
        self.bow(p, k)
        yawn = bump(self.t, 1.0, 0.8)
        p.mouth = yawn
        p.eyes = "closed" if yawn > 0.3 else "open"
        p.ears = -0.5 * yawn
        p.tail, p.tail_curl = 50 * k, 0.3
        if self.done:
            self.go_idle()

    def _meow(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.pose_rest(p)
        if self.entered:
            self.say(self.rng.choice(self.LINES), 1.2)
        m = bump(self.t, 0.35, 0.5)
        p.mouth = 0.8 * m
        p.head = (1.0, -2.0 * m)
        p.head_angle = -15 * m
        p.tail = 45
        if self.done:
            self.go_idle()

    def _pounce(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """Stalk the cursor low and slow, wiggle the hips, leap at it."""
        if self.paused or self.cursor is None:
            self.pose_rest(p)
            self.go_idle()
            return
        if self.entered:
            self._wiggle_at = None
        if self._wiggle_at is None:
            left = self.stalk(p, b, dt, 28.0)
            p.tail, p.tail_curl = -5.0, 0.2
            p.tail_wag = 10 * math.sin(self.clock * 8)    # twitchy tail tip
            if left < 110 or self.t > 3.5:
                self._wiggle_at = self.t
            return
        # hips wiggle, then launch
        w = self.t - self._wiggle_at
        r = self.rig
        p.shoulder = (0.0, 0.4 * -r.shoulder[1])
        p.hip = (2.0 * math.sin(self.clock * 30), 0.3 * -r.hip[1] - 2)
        p.head = (4.0, 0.3 * -r.shoulder[1])
        p.ears, p.eyes = 1.0, "wide"
        p.tail, p.tail_wag = -5.0, 14 * math.sin(self.clock * 30)
        if w > 0.6:
            dx = self.cursor[0]
            self.facing = 1 if dx >= 0 else -1
            self.vx = clamp(dx * 1.6, -520.0, 520.0)
            self.vy = self.JUMP_VY
            self._air_kind = "jump"
            self.say("!", 0.6)
            self.go("air")

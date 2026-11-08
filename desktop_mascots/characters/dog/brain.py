"""Dogs: one shared brain, two breeds.

Labrador: easy-going, pants, begs, naps a lot.
Border collie: high energy, more zoomies, and "herds" your cursor with the
classic low, staring stalk.
"""
from __future__ import annotations

import math

from ...engine import Bounds, bump, ease
from ...quadruped.brain import FF, FN, QuadBrain, QuadPose, Rig

LAB_RIG = Rig(shoulder=(13.0, -29.0), hip=(-14.0, -30.0), front_leg=(14.0, 15.0), hind_leg=(15.0, 15.0),
              head=(27.0, -44.0), neck_base=(3.0, -6.0), tail_root=(-8.0, -4.0), foot_y=-2.3)
COLLIE_RIG = Rig(shoulder=(13.0, -27.0), hip=(-15.0, -28.0), front_leg=(13.0, 14.0), hind_leg=(14.0, 14.0),
                 head=(26.0, -42.0), neck_base=(3.0, -6.0), tail_root=(-8.0, -4.0), foot_y=-2.2)
BARK_TIME = 1.2


class DogBrain(QuadBrain):
    WALK_SPEED = 60.0
    STRIDE = 7.0
    LIFT = 4.0
    RUN_SPEED = 260.0
    PANTS = False
    BARKS: tuple[str, ...] = ("WOOF!", "Woof woof!")

    def poke(self) -> None:
        """Poke a dog and it barks back, then wags."""
        if self.state == "nap":
            self.say("…?")
            self.go_idle()
        elif not self.airborne and self.state not in ("crouch", "land", "dizzy"):
            self.go("bark", BARK_TIME)
        else:
            self.say(self.rng.choice(self.LINES))

    def pose_rest(self, p: QuadPose) -> None:
        self.breathe(p)
        p.tail_wag = 12 * math.sin(self.clock * 3)
        if self.PANTS:
            p.mouth = 0.3 + 0.08 * math.sin(self.clock * 9)
            p.tongue = True

    # ----------------------------------------------------------- dog states
    def _bark(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        if self.entered:
            self.say(self.rng.choice(self.BARKS), 1.3)
        woof = bump(self.t, 0.25, 0.3) + bump(self.t, 0.7, 0.3)
        p.mouth = 0.9 * woof
        p.head = (2.0, -3.0 - 2 * woof)
        p.head_angle = -10 * woof
        p.shoulder = (p.shoulder[0], p.shoulder[1] - 2 * woof)
        p.ears = 1.0
        p.tail = 15
        p.tail_wag = 25 * math.sin(self.clock * 18)
        if self.done:
            self.go("wag", 1.5)

    def _wag(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        fast = math.sin(self.clock * 22)
        p.tail = 20
        p.tail_wag = 35 * fast
        p.hip = (p.hip[0] + 1.2 * fast, p.hip[1])   # the whole back end wags along
        p.mouth, p.tongue = 0.45, True
        if int(self.clock * 1.5) % 3 == 0:
            p.eyes = "happy"
        if self.done:
            self.go_idle()

    def _bow(self, dt: float, b: Bounds, p: QuadPose) -> None:
        self.breathe(p)
        k = ease(self.t / 0.3) * ease((self.duration - self.t) / 0.3)
        self.bow(p, k)
        p.tail_wag = 30 * math.sin(self.clock * 20)
        p.ears = 1.0
        p.mouth, p.tongue = 0.35, True
        if self.entered and self.rng.random() < 0.5:
            self.say("Play?", 1.2)
        if self.done:
            # play bows usually end in zoomies
            if self.rng.random() < 0.6 and not self.paused:
                self.facing = self.rng.choice((-1, 1))
                self.go("run", self.rng.uniform(1.5, 3.0))
            else:
                self.go_idle()


class LabradorBrain(DogBrain):
    rig = LAB_RIG
    PANTS = True
    LINES = ("Woof!", "Woof woof!", "*pant pant*", "Treat?")
    CHOICES = (
        (24, "walk", (2.5, 6.0)),
        (7, "run", (1.8, 3.0)),
        (11, "sit", (2.5, 5.0)),
        (9, "nap", (6.0, 12.0)),
        (11, "sniff", (2.0, 4.5)),
        (6, "bark", (BARK_TIME, BARK_TIME)),
        (8, "wag", (1.5, 3.0)),
        (5, "bow", (1.2, 2.0)),
        (7, "beg", (2.0, 3.5)),
        (4, "crouch", (0.14, 0.14)),
        (8, "idle", (1.5, 3.5)),
    )

    def _beg(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """Sit up with both front paws in the air."""
        self.pose_rest(p)
        k = ease(self.t / 0.4) * ease((self.duration - self.t) / 0.4)
        self.sit(p, k)
        p.shoulder = (p.shoulder[0] - 5 * k, p.shoulder[1] - 7 * k)
        wave = math.sin(self.clock * 7)
        if k > 0.5:
            p.feet = ((4 + 2 * wave, 11, True), (5 - 2 * wave, 10, True), p.feet[2], p.feet[3])
        p.head = (p.head[0] - 3 * k, p.head[1] - 5 * k)
        p.head_angle = -12 * k
        p.ears = 0.5
        p.tail_wag = 25 * math.sin(self.clock * 16)
        if self.entered:
            self.say("Treat? Treat?", 1.6)
        if self.done:
            self.go_idle()


class CollieBrain(DogBrain):
    rig = COLLIE_RIG
    RUN_SPEED = 330.0
    BARKS = ("ARF!", "Arf arf!")
    LINES = ("Arf!", "Arf arf!", "Woof!", "Ball?")
    CHOICES = (
        (22, "walk", (2.0, 5.0)),
        (13, "run", (2.0, 3.5)),
        (8, "sit", (2.0, 4.0)),
        (4, "nap", (4.0, 8.0)),
        (9, "sniff", (1.5, 3.5)),
        (7, "bark", (BARK_TIME, BARK_TIME)),
        (6, "wag", (1.2, 2.5)),
        (7, "bow", (1.0, 1.6)),
        (11, "herd", (4.0, 7.0)),
        (5, "crouch", (0.14, 0.14)),
        (8, "idle", (1.2, 3.0)),
    )

    def _herd(self, dt: float, b: Bounds, p: QuadPose) -> None:
        """The border-collie eye: creep low toward the cursor, then dash."""
        if self.paused or self.cursor is None:
            self.pose_rest(p)
            self.go_idle()
            return
        left = self.stalk(p, b, dt, 22.0)
        p.head = (6.0, p.head[1] + 4)       # head lower than the back, staring
        p.tail = -25
        if left < 70 or self.done:
            self.go("run", 1.4)

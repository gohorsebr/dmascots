"""A single frame of the monkey's body language.

Kept free of Qt so the behaviour code (brain.py) can be simulated headlessly.

Angles are in degrees, measured from "hanging straight down", positive toward
the direction the monkey faces. Limbs are (upper_segment, lower_relative).
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass
class Pose:
    bob: float = 0.0            # vertical body offset, px (negative = up)
    squash: float = 1.0         # <1 squashed, >1 stretched (pivot: feet)
    lean: float = 0.0           # whole-body tilt, positive = forward
    swing: float = 0.0          # pendulum swing around the scruff (when dragged)
    head_tilt: float = 0.0
    arm_front: tuple[float, float] = (12.0, 10.0)
    arm_back: tuple[float, float] = (-12.0, -10.0)
    leg_front: tuple[float, float] = (3.0, 0.0)
    leg_back: tuple[float, float] = (-3.0, 0.0)
    tail: float = 0.0           # sway, -1..1
    eyes: str = "open"          # open | wide | closed | happy | dizzy
    look: tuple[float, float] = (0.0, 0.0)  # pupil direction (screen space), -1..1
    mouth: str = "smile"        # smile | big | o | chew
    banana: int = -1            # -1 = no banana, otherwise bites taken (0..3)
    stars: float | None = None  # dizzy stars orbit angle (radians)
    bubble: str | None = None   # speech bubble text
    grounded: bool = True       # keep the lowest foot planted on the floor

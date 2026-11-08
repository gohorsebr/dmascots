from ...engine import Character
from . import art
from .brain import WHISTLE_TIME, ParrotBrain

CHARACTER = Character(
    name="parrot",
    brain=ParrotBrain,
    paint=art.paint,
    half_width=art.HALF_WIDTH,
    height=art.HEIGHT,
    eye_y=art.EYE_Y,
    icon_box=(9, -56, 38),
    previews=(
        ("dance", "dance", 4.0, 0.5),
        ("mimic", "mimic", 2.0, 0.4),
        ("whistle", "whistle", WHISTLE_TIME, 0.6),
        ("preen", "preen", 3.0, 1.2),
        ("fly", "fly", 1.0, 0.45),
    ),
)

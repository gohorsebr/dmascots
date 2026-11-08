from ...engine import Character
from . import art
from .brain import EAT_TIME, MonkeyBrain

CHARACTER = Character(
    name="monkey",
    brain=MonkeyBrain,
    paint=art.paint,
    half_width=art.HALF_WIDTH,
    height=art.HEIGHT,
    eye_y=art.EYE_Y,
    icon_box=(art.FACE_DX / 2, -80, 64),
    previews=(
        ("scratch", "scratch", 3.0, 1.0),
        ("eat: bite", "eat", EAT_TIME, 1.2),
        ("eat: chew", "eat", EAT_TIME, 2.7),
        ("eat: done", "eat", EAT_TIME, 4.5),
    ),
)

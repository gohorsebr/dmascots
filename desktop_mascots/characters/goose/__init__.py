from ...engine import Character
from . import art
from .brain import HONK_TIME, GooseBrain

CHARACTER = Character(
    name="goose",
    brain=GooseBrain,
    paint=art.paint,
    half_width=art.HALF_WIDTH,
    height=art.HEIGHT,
    eye_y=art.EYE_Y,
    grip=(36.0, -76.0),
    icon_box=(24, -76, 42),
    previews=(
        ("graze", "graze", 4.0, 1.0),
        ("honk", "honk", HONK_TIME, 0.3),
        ("preen", "preen", 3.0, 1.2),
        ("chase the cursor", "chase", 4.0, 0.3),
    ),
)

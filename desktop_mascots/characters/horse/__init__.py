from ...engine import Character
from .art import HorseArt
from .brain import NEIGH_TIME, REAR_TIME, HorseBrain

CHARACTER = Character(
    name="horse",
    brain=HorseBrain,
    paint=HorseArt().paint,
    half_width=88.0,
    height=130.0,
    eye_y=102.0,
    icon_box=(70, -95, 72),
    size=(300, 280),
    previews=(
        ("trot", "trot", 4.0, 0.3),
        ("gallop", "run", 3.0, 0.8),
        ("graze", "graze", 5.0, 2.0),
        ("neigh", "neigh", NEIGH_TIME, 0.6),
        ("rear", "rear", REAR_TIME, 0.9),
        ("tail swish", "swish", 3.0, 0.5),
    ),
)

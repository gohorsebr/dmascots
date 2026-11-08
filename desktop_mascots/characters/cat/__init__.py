from ...engine import Character
from .art import CatArt
from .brain import MEOW_TIME, CatBrain

CHARACTER = Character(
    name="cat",
    brain=CatBrain,
    paint=CatArt().paint,
    half_width=46.0,
    height=52.0,
    eye_y=36.0,
    icon_box=(24, -37, 34),
    previews=(
        ("sit (slow blink)", "sit", 5.0, 1.0),
        ("groom: lick paw", "groom", 4.0, 0.9),
        ("groom: wipe face", "groom", 4.0, 1.5),
        ("nap", "nap", 8.0, 2.0),
        ("stretch + yawn", "stretch", 2.0, 1.0),
        ("meow", "meow", MEOW_TIME, 0.35),
        ("stalk the cursor", "pounce", 6.0, 0.8),
        ("zoomies", "run", 2.0, 0.6),
    ),
)

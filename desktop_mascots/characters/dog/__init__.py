from ...engine import Character
from .art import CollieArt, LabradorArt
from .brain import BARK_TIME, CollieBrain, LabradorBrain

_DOG = dict(half_width=50.0, height=62.0, eye_y=47.0, icon_box=(30, -45, 40), grip=(44.0, -38.0))
_COMMON = (
    ("sit", "sit", 4.0, 1.0),
    ("nap", "nap", 8.0, 2.0),
    ("sniff", "sniff", 3.0, 1.0),
    ("bark", "bark", BARK_TIME, 0.25),
    ("wag", "wag", 3.0, 0.5),
    ("play bow", "bow", 2.0, 0.8),
    ("zoomies", "run", 3.0, 0.8),
)

LABRADOR = Character(name="labrador", brain=LabradorBrain, paint=LabradorArt().paint,
                     previews=_COMMON + (("beg", "beg", 3.0, 1.2),), **_DOG)
COLLIE = Character(name="collie", brain=CollieBrain, paint=CollieArt().paint,
                   previews=_COMMON + (("herd the cursor", "herd", 6.0, 0.8),), **_DOG)

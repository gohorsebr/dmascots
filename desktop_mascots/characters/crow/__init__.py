from ...engine import Character
from . import art
from .brain import CAW_TIME, SHINY_TIME, CrowBrain

CHARACTER = Character(
    name="crow",
    brain=CrowBrain,
    paint=art.paint,
    half_width=art.HALF_WIDTH,
    height=art.HEIGHT,
    eye_y=art.EYE_Y,
    icon_box=(22, -54, 40),
    previews=(
        ("peck", "peck", 3.0, 1.0),
        ("caw", "caw", CAW_TIME, 0.3),
        ("curious tilt", "tilt", 3.0, 0.5),
        ("shiny: pick up", "shiny", SHINY_TIME, 0.85),
        ("shiny: admire", "shiny", SHINY_TIME, 1.7),
        ("shiny: hide it", "shiny", SHINY_TIME, 3.95),
        ("fly", "fly", 1.0, 0.45),
    ),
)

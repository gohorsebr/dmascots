"""Registry of available mascots. To add one, create a subpackage exposing CHARACTER."""
from ..engine import Character
from .cat import CHARACTER as CAT
from .crow import CHARACTER as CROW
from .dog import COLLIE, LABRADOR
from .goose import CHARACTER as GOOSE
from .monkey import CHARACTER as MONKEY

CHARACTERS: dict[str, Character] = {c.name: c for c in (MONKEY, GOOSE, CROW, LABRADOR, COLLIE, CAT)}

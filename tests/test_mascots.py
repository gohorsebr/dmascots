"""Headless behaviour tests for every mascot: python -m unittest discover tests"""
import random
import unittest

from desktop_mascots.characters import CHARACTERS
from desktop_mascots.engine import Bounds

DT = 1 / 60
B = Bounds(left=50, right=950, top=150, floor=800)


def step(brain, seconds, bounds=B, cursor=(30, -40)):
    seen = set()
    for _ in range(round(seconds / DT)):
        brain.update(DT, bounds, cursor=cursor)
        seen.add(brain.state)
    return seen


class MascotTest(unittest.TestCase):
    def each(self):
        for name, char in CHARACTERS.items():
            with self.subTest(mascot=name):
                yield char

    def test_wanders_inside_bounds_and_uses_every_behaviour(self):
        for char in self.each():
            b = char.brain(500, B.floor - 10, rng=random.Random(7))
            seen = set()
            for i in range(round(240 / DT)):
                # sweep the cursor around so cursor-driven behaviours get exercised
                b.update(DT, B, cursor=(300 if (i // 300) % 2 else -300, -40))
                seen.add(b.state)
                if not b.airborne:
                    self.assertEqual(b.y, B.floor)
                    self.assertTrue(B.left <= b.x <= B.right, b.x)
            expected = {state for _, state, _ in char.brain.CHOICES} | {"air", "land"}
            self.assertTrue(expected <= seen, f"never entered: {expected - seen}")

    def test_drag_and_throw(self):
        for char in self.each():
            b = char.brain(500, B.floor, rng=random.Random(1))
            step(b, 1)
            b.grab()
            for i in range(30):
                b.drag_to(500 + i * 25, 300)
                b.update(DT, B)
            self.assertEqual(b.state, "dragged")
            self.assertNotEqual(b.pose.swing, 0)
            b.release()
            self.assertEqual(b.state, "air")
            self.assertGreater(b.vx, 0)
            step(b, 3)
            self.assertFalse(b.airborne)
            self.assertEqual(b.y, B.floor)

    def test_hard_drop_makes_it_dizzy(self):
        tall = Bounds(left=50, right=950, top=0, floor=1200)
        for char in self.each():
            b = char.brain(500, 0, rng=random.Random(2))
            self.assertIn("dizzy", step(b, 1.5, tall))

    def test_paused_stays_put(self):
        for char in self.each():
            b = char.brain(500, B.floor, rng=random.Random(4))
            step(b, 1)
            b.paused = True
            step(b, 0.1)
            x = b.x
            step(b, 30, cursor=(400, 0))
            self.assertAlmostEqual(b.x, x)

    def test_poke_reacts_and_talks(self):
        for char in self.each():
            b = char.brain(500, B.floor, rng=random.Random(5))
            step(b, 1)
            b.go("idle", 99)
            b.poke()
            self.assertNotEqual(b.state, "idle")
            step(b, 0.1)
            self.assertIn(b.pose.bubble, char.brain.LINES)


if __name__ == "__main__":
    unittest.main()

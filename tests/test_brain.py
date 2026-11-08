"""Headless behaviour tests: python -m unittest discover tests"""
import random
import unittest

from monkey_mascot.brain import Bounds, Brain

DT = 1 / 60
B = Bounds(left=50, right=950, top=150, floor=800)


def step(brain: Brain, seconds: float, bounds: Bounds = B) -> set:
    seen = set()
    for _ in range(round(seconds / DT)):
        brain.update(DT, bounds, cursor=(30, -40))
        seen.add(brain.state)
    return seen


class BrainTest(unittest.TestCase):
    def test_drops_in_and_wanders_inside_bounds(self):
        b = Brain(500, 100, rng=random.Random(7))
        seen = step(b, 0.05)
        for _ in range(round(180 / DT)):
            b.update(DT, B)
            seen.add(b.state)
            if b.state not in ("air", "dragged"):
                self.assertEqual(b.y, B.floor)
                self.assertTrue(B.left <= b.x <= B.right, b.x)
        self.assertTrue({"air", "dizzy", "idle", "walk", "scratch", "eat", "crouch", "land"} <= seen, seen)

    def test_drag_and_throw(self):
        b = Brain(500, B.floor, rng=random.Random(1))
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
        self.assertNotIn(b.state, ("air", "dragged"))
        self.assertEqual(b.y, B.floor)

    def test_hard_drop_makes_it_dizzy(self):
        tall = Bounds(left=50, right=950, top=0, floor=1200)
        b = Brain(500, 0, rng=random.Random(2))
        seen = step(b, 1.5, tall)
        self.assertIn("dizzy", seen)

    def test_paused_stays_put(self):
        b = Brain(500, B.floor, rng=random.Random(4))
        step(b, 1)
        b.paused = True
        step(b, 0.1)
        x = b.x
        step(b, 30)
        self.assertAlmostEqual(b.x, x)

    def test_poke_jumps_and_talks(self):
        b = Brain(500, B.floor, rng=random.Random(5))
        step(b, 1)
        b._go("idle", 99)
        b.poke()
        seen = step(b, 0.3)
        self.assertIn("air", seen)
        self.assertIsNotNone(b.pose.bubble)


if __name__ == "__main__":
    unittest.main()

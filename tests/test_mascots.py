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
    def each(self, check):
        """Run check(character) for every mascot, reporting each one separately."""
        for name, char in CHARACTERS.items():
            with self.subTest(mascot=name):
                check(char)

    def test_wanders_inside_bounds_and_uses_every_behaviour(self):
        def check(char):
            b = char.brain(500, B.floor - 10, rng=random.Random(7))
            expected = {state for _, state, _ in char.brain.CHOICES} | {"air", "land"}
            seen = set()
            for i in range(round(900 / DT)):   # stop as soon as everything has happened
                # sweep the cursor around so cursor-driven behaviours get exercised
                b.update(DT, B, cursor=(300 if (i // 300) % 2 else -300, -40))
                seen.add(b.state)
                if not b.airborne:
                    self.assertEqual(b.y, B.floor, b.state)
                    self.assertTrue(B.left <= b.x <= B.right, b.x)
                if expected <= seen:
                    break
            self.assertTrue(expected <= seen, f"never entered: {expected - seen}")
        self.each(check)

    def test_drag_and_throw(self):
        def check(char):
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
        self.each(check)

    def test_hard_drop_makes_it_dizzy(self):
        tall = Bounds(left=50, right=950, top=0, floor=1200)

        def check(char):
            b = char.brain(500, 0, rng=random.Random(2))
            self.assertIn("dizzy", step(b, 1.5, tall))
        self.each(check)

    def test_paused_stays_put(self):
        def check(char):
            b = char.brain(500, B.floor, rng=random.Random(4))
            step(b, 1)
            b.paused = True
            step(b, 0.1)
            x = b.x
            step(b, 30, cursor=(400, 0))
            self.assertAlmostEqual(b.x, x)
        self.each(check)

    def test_poke_reacts_and_talks(self):
        def check(char):
            b = char.brain(500, B.floor, rng=random.Random(5))
            step(b, 1)
            b.go("idle", 99)
            b.poke()
            self.assertNotEqual(b.state, "idle")
            step(b, 0.1)
            self.assertTrue(b.pose.bubble)
        self.each(check)

    def test_sent_message_is_not_overwritten_by_chatter(self):
        def check(char):
            b = char.brain(500, B.floor, rng=random.Random(6))
            step(b, 1)
            b.say("build finished", 3.0, important=True)
            b.poke()                     # pokes, barks and honks would normally talk
            step(b, 1.0)
            self.assertEqual(b.pose.bubble, "build finished")
            step(b, 2.5)
            self.assertNotEqual(b.pose.bubble, "build finished")
        self.each(check)


if __name__ == "__main__":
    unittest.main()

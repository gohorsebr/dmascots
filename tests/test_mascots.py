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

    def test_parrot_repeats_what_it_was_told(self):
        b = CHARACTERS["parrot"].brain(500, B.floor, rng=random.Random(8))
        step(b, 1)
        b.say("deploy is green", 0.5, important=True)
        step(b, 1)
        said = set()
        for _ in range(20):
            b.go("mimic", 2.0)
            step(b, 0.1)
            said.add(b.pose.bubble)
        self.assertIn("deploy is green", said)

    def test_steals_the_pointer_and_gives_it_back(self):
        def check(char):
            b = char.brain(300, B.floor, rng=random.Random(9))
            step(b, 1)
            b.go("idle", 99)
            cursor = [800.0, 300.0]          # absolute pointer position, far away
            phases, held = [], 0.0
            self.assertTrue(b.steal(2.0))
            for _ in range(round(12 / DT)):
                b.update(DT, B, cursor=(cursor[0] - b.x, cursor[1] - (b.y - char.eye_y)))
                if b.pointer:                # the window would now move the real pointer
                    gx, gy = char.grip
                    cursor = [b.x + gx * b.facing, b.y + gy]
                    if b.pointer[0] == "hold":
                        held += DT
                self.assertTrue(B.left <= b.x <= B.right)
                if not phases or phases[-1] != (b.pointer or ("free",))[0]:
                    phases.append((b.pointer or ("free",))[0])
                if b.state != "steal":
                    break
            self.assertEqual(phases, ["free", "pull", "hold", "free"])   # chase, snatch, carry, give back
            self.assertAlmostEqual(held, 2.0, delta=0.1)
            self.assertIsNone(b.pointer)
            self.assertIn(b.pose.bubble, char.brain.RETURN_LINES)
        self.each(check)

    def test_clicking_the_thief_gets_the_pointer_back(self):
        def check(char):
            b = char.brain(500, B.floor, rng=random.Random(10))
            step(b, 1)
            b.steal(30.0)
            step(b, 2.0, cursor=(0, 0))      # pointer is right there: snatched and carried
            self.assertEqual(b.pointer, ("hold", 1.0))
            b.poke()
            step(b, 0.05)
            self.assertIsNone(b.pointer)
            self.assertNotEqual(b.state, "steal")
        self.each(check)

    def test_steal_requested_mid_air_waits_for_landing(self):
        b = CHARACTERS["goose"].brain(500, B.floor - 300, rng=random.Random(11))
        b.steal(1.0)
        self.assertEqual(b.state, "air")
        seen = step(b, 2.0, cursor=(0, 0))
        self.assertIn("steal", seen)


if __name__ == "__main__":
    unittest.main()

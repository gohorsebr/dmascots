"""The running app's command handling (list/add/remove/say/steal), with real windows offscreen."""
import os
import types
import unittest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication  # noqa: E402

APP = QApplication.instance() or QApplication([])

from desktop_mascots.app import Herd  # noqa: E402
from desktop_mascots.characters import CHARACTERS  # noqa: E402


class HerdTest(unittest.TestCase):
    def setUp(self):
        self.herd = Herd(APP, CHARACTERS, scale=1.0, managed=False, tray=False)
        self.quits = []
        self.herd.app = types.SimpleNamespace(quit=lambda: self.quits.append(1))

    def tearDown(self):
        for w in list(self.herd.windows):
            self.herd.remove(w)

    def cmd(self, **request):
        reply = self.herd.handle(request)
        self.assertTrue(reply["ok"], reply)
        return reply

    def ids(self, reply):
        return [m["id"] for m in reply["mascots"]]

    def test_add_and_list(self):
        added = self.cmd(cmd="spawn", mascots=["goose", "cat", "goose"])
        self.assertEqual([m["name"] for m in added["mascots"]], ["goose", "cat", "goose"])
        self.assertEqual(self.ids(added), [1, 2, 3])
        listed = self.cmd(cmd="list")["mascots"]
        self.assertEqual([(m["id"], m["name"]) for m in listed], [(1, "goose"), (2, "cat"), (3, "goose")])
        self.assertTrue(all({"state", "x", "y", "leaves_in", "holding_pointer"} <= m.keys() for m in listed))

    def test_remove_by_id_name_and_all(self):
        self.cmd(cmd="spawn", mascots=["goose", "cat", "goose", "horse"])
        self.assertEqual(self.ids(self.cmd(cmd="remove", targets=["2"])), [2])
        self.assertEqual(self.ids(self.cmd(cmd="remove", targets=["goose"])), [1, 3])
        self.assertEqual(self.cmd(cmd="remove", targets=["7", "parrot"])["count"], 0)
        self.assertEqual(self.ids(self.cmd(cmd="remove", targets=["all"])), [4])
        self.assertEqual(self.cmd(cmd="list")["mascots"], [])
        self.assertEqual(self.quits, [])          # an empty herd keeps running
        self.assertEqual(self.ids(self.cmd(cmd="spawn", mascots=["crow"])), [5])   # ids never reused

    def test_app_started_with_time_exits_when_empty(self):
        self.herd.quit_when_empty = True
        self.cmd(cmd="spawn", mascots=["cat"])
        self.cmd(cmd="remove", targets=["all"])
        self.assertEqual(self.quits, [1])

    def test_say_and_steal_target_one_mascot_by_id(self):
        self.cmd(cmd="spawn", mascots=["goose", "goose"])
        first, second = self.herd.windows
        self.assertEqual(self.cmd(cmd="say", text="just you", to="2")["count"], 1)
        self.assertIsNone(first.brain._bubble)
        self.assertEqual(second.brain._bubble, "just you")
        stolen = self.cmd(cmd="steal", to="1", seconds=3)
        self.assertEqual(self.ids(stolen), [1])
        # just spawned, so it is still dropping in: the theft starts once it lands
        self.assertTrue(first.brain.state == "steal" or first.brain._pending_steal is not None)
        self.assertFalse(self.herd.handle({"cmd": "steal", "to": "2"})["ok"])   # one thief at a time

    def test_lifetime_shows_in_list(self):
        self.cmd(cmd="spawn", mascots=["parrot"], lifetime=30)
        (m,) = self.cmd(cmd="list")["mascots"]
        self.assertAlmostEqual(m["leaves_in"], 30, delta=1)

    def test_click_through_applies_to_new_mascots(self):
        self.herd.through.setChecked(True)
        self.cmd(cmd="spawn", mascots=["monkey"])
        self.assertTrue(self.herd.windows[0].click_through)


if __name__ == "__main__":
    unittest.main()

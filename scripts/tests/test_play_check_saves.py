import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "play_check_saves.py"


def load(data: Path):
    os.environ["MARQUEE_DATA_DIR"] = str(data)
    try:
        spec = importlib.util.spec_from_file_location("play_check_saves", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        del os.environ["MARQUEE_DATA_DIR"]


class PlayCheckSaves(unittest.TestCase):
    def test_restore_returns_originals_and_keeps_the_runs_saves(self):
        with tempfile.TemporaryDirectory() as raw:
            data, out = Path(raw, "data"), Path(raw, "out")
            card = data / "pcsx2/xdg-config/PCSX2/memcards"
            card.mkdir(parents=True)
            (card / "Mcd001.ps2").write_text("owner")
            out.mkdir()
            saves = load(data)
            saves.snapshot(out)
            (card / "Mcd001.ps2").write_text("play check")
            # A save location the run created from nothing is removed again.
            fresh = data / "shadps4/xdg-data/shadPS4/home/1000/savedata"
            fresh.mkdir(parents=True)
            (fresh / "slot").write_text("new")
            saves.restore(out)
            self.assertEqual((card / "Mcd001.ps2").read_text(), "owner")
            self.assertFalse(fresh.exists())
            kept = out / "saves-after/pcsx2/xdg-config/PCSX2/memcards/Mcd001.ps2"
            self.assertEqual(kept.read_text(), "play check")


if __name__ == "__main__":
    unittest.main()

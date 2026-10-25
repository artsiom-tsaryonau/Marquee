import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("storage_migrate", Path(__file__).parents[1] / "storage-migrate.py")
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)


class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.source = self.base / "old"
        self.source.mkdir()
        (self.source / "game.bin").write_bytes(b"game data")
        (self.source / "relative-link").symlink_to("game.bin")
        self.root = self.base / "collection"
        self.journal = storage.plan(self.root, [f"{self.source}=library/imported"])
        self.destination = self.root / "library/imported"

    def test_move_preserves_identity_links_and_rollback(self):
        inode = (self.source / "game.bin").stat().st_ino
        storage.migrate(self.journal)
        self.assertTrue(self.source.is_symlink())
        self.assertEqual((self.source / "relative-link").read_bytes(), b"game data")
        self.assertEqual((self.destination / "game.bin").stat().st_ino, inode)
        storage.migrate(self.journal)  # Idempotent completion.
        storage.migrate(self.journal, rollback=True)
        self.assertFalse(self.source.is_symlink())
        self.assertFalse(self.destination.exists())

    def test_recovery_after_rename_before_journal_update(self):
        self.destination.parent.mkdir(parents=True)
        self.source.rename(self.destination)
        storage.migrate(self.journal)
        self.assertTrue(self.source.is_symlink())
        self.assertEqual(json.loads(self.journal.read_text())["entries"][0]["stage"], "verified")

    def test_changed_source_is_not_moved(self):
        (self.source / "game.bin").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            storage.migrate(self.journal)
        self.assertFalse(self.source.is_symlink())

    def test_destination_collision_is_not_adopted(self):
        self.destination.mkdir(parents=True)
        with self.assertRaises(ValueError):
            storage.migrate(self.journal)
        self.assertFalse(self.source.is_symlink())

    def test_changed_destination_is_not_rolled_back(self):
        storage.migrate(self.journal)
        (self.destination / "game.bin").write_bytes(b"new save")
        with self.assertRaises(ValueError):
            storage.migrate(self.journal, rollback=True)
        self.assertTrue(self.source.is_symlink())

    def test_external_link_is_not_followed(self):
        external = self.base / "outside"
        external.mkdir()
        (external / "secret").write_text("not part of inventory")
        (self.source / "external").symlink_to(external)
        rows = storage.inventory(self.source)
        self.assertFalse(any("secret" in row["path"] for row in rows))

    def test_unknown_cohort_is_rejected(self):
        with self.assertRaises(ValueError):
            storage.migrate(self.journal, only_source=self.base / "typo")

    def test_missing_root_is_rejected(self):
        with self.assertRaises(ValueError):
            storage.inventory(self.base / "missing")


if __name__ == "__main__":
    unittest.main()

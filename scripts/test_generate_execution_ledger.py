import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("generate-execution-ledger.py")
SPEC = importlib.util.spec_from_file_location("generate_execution_ledger", SCRIPT)
LEDGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LEDGER)


class BinaryBaselineInventoryTests(unittest.TestCase):
    def test_discovers_nested_active_binary_baselines_and_omits_frozen_history(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository_root = Path(temporary_directory)
            baseline_root = repository_root / "tests/functional/baselines"
            files = (
                "functional/one.tui.pb",
                "showcase/buttons/held.tui.pb",
                "pre_refactor/showcase/buttons/held.tui.pb",
            )
            for relative in files:
                path = baseline_root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixture")

            json_baseline = baseline_root / "showcase/buttons/held.tui.json"
            json_baseline.write_text("{}")

            inventory = LEDGER.binary_baseline_inventory(baseline_root, repository_root)

        self.assertEqual(
            inventory,
            [
                {
                    "path": "tests/functional/baselines/functional/one.tui.pb",
                    "test_id": "functional/one",
                    "active": True,
                },
                {
                    "path": "tests/functional/baselines/showcase/buttons/held.tui.pb",
                    "test_id": "showcase/buttons/held",
                    "active": True,
                },
            ],
        )


if __name__ == "__main__":
    unittest.main()

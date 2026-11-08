from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str, path: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


coverage = load_script("check_refactor_coverage", "scripts/check-refactor-coverage.py")
acceptance = load_script("accept_baseline", "scripts/accept-baseline.py")


class EnumVariantParserTests(unittest.TestCase):
    def test_reads_variants_and_ignores_comments_and_non_variant_lines(self):
        self.assertEqual(
            coverage.enum_variants(
                "\n    Alpha, // primary\n    Beta\n    // comment\n"
                "    #[cfg(test)]\n    value = 3,\n"
            ),
            ["Alpha", "Beta"],
        )

    def test_long_non_variant_line_is_rejected_without_regex_backtracking(self):
        self.assertEqual(coverage.enum_variants("    " + "A" * 100_000 + " value\n"), [])


class AtomicReceiptWriteTests(unittest.TestCase):
    def test_preexisting_predictable_temp_symlink_is_not_followed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / "receipt.json"
            outside = root / "outside.json"
            predictable_temp = root / "receipt.json.tmp"
            outside.write_text("preserve me", encoding="utf-8")
            predictable_temp.symlink_to(outside)

            acceptance.atomic_write_text(destination, '{"accepted": true}\n')

            self.assertEqual(destination.read_text(encoding="utf-8"), '{"accepted": true}\n')
            self.assertEqual(outside.read_text(encoding="utf-8"), "preserve me")
            self.assertTrue(predictable_temp.is_symlink())

    def test_failed_replace_removes_the_exclusive_temporary_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / "receipt.json"
            with mock.patch.object(acceptance.os, "replace", side_effect=OSError("replace failed")):
                with self.assertRaises(OSError):
                    acceptance.atomic_write_text(destination, "receipt")

            self.assertFalse(destination.exists())
            self.assertEqual(list(root.iterdir()), [])


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""The checklist generator, checked: against the catalogue in the tree and
against a small synthetic one.  Standard library only; `crates/check.sh`
runs it."""
import importlib.util
import json
import os
import tempfile
import unittest

# the generator's file name has a hyphen, so it is loaded by path
_spec = importlib.util.spec_from_file_location(
    "field_checklist",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "field-checklist.py"),
)
fc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fc)


def entry(i, kind, **over):
    e = {
        "id": i, "area": "x", "gap": "x", "title": f"Title of {i}", "milestone": "manual",
        "kind": kind, "spec": [], "rule": [], "given": "g", "when": f"When {i}",
        "then": f"Then {i}", "oracle": "behaviour", "interpretation": None,
    }
    e.update(over)
    return e


class AgainstTheTree(unittest.TestCase):
    def setUp(self):
        with open(fc.CATALOGUE, encoding="utf-8") as f:
            self.entries = json.load(f)

    def test_every_manual_entry_and_the_two_device_three_are_rows(self):
        want = sorted(e["id"] for e in self.entries if e["kind"] == "manual")
        ids = [e["id"] for e in fc.rows(self.entries)]
        self.assertEqual(ids[: len(want)], want)
        self.assertEqual(ids[len(want):], fc.TWO_DEVICE)
        self.assertEqual(len(ids), len(set(ids)), "no row twice")

    def test_a_withdrawn_product_row_is_not_a_row(self):
        withdrawn = [e["id"] for e in self.entries if e["kind"] == "withdrawn"]
        ids = {e["id"] for e in fc.rows(self.entries)}
        for w in withdrawn:
            self.assertNotIn(w, ids)

    def test_each_row_carries_id_title_when_then_marks_and_notes(self):
        text = fc.render(self.entries, run="abc1234-dry-1")
        chosen = fc.rows(self.entries)
        self.assertIn(f"{len(chosen)} rows.", text)
        self.assertIn("`abc1234-dry-1`", text)
        for e in chosen:
            self.assertIn(f"## {e['id']}  {e['title']}", text)
            self.assertIn(f"Do: {e['when']}", text)
            self.assertIn(f"Look for: {e['then']}", text)
        self.assertEqual(text.count(fc.MARKS), len(chosen))
        self.assertEqual(text.count("Notes: ___"), len(chosen))

    def test_the_main_writes_a_file(self):
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "checklist.md")
            self.assertEqual(fc.main(["-o", out, "--run", "r"]), 0)
            with open(out, encoding="utf-8") as f:
                self.assertTrue(f.read().startswith("# Field-test checklist: r"))


class Synthetic(unittest.TestCase):
    def catalogue(self):
        return [
            entry("PRD-02", "manual"),
            entry("PRD-01", "withdrawn", then="Withdrawn.", when="Withdrawn."),
            entry("TRV-12", "positive", milestone="after-5"),
            entry("TRV-13", "negative", milestone="after-5"),
            entry("MET-12", "positive", milestone="after-5"),
            entry("ARC-01", "positive", milestone=3),
        ]

    def test_order_and_selection(self):
        ids = [e["id"] for e in fc.rows(self.catalogue())]
        self.assertEqual(ids, ["PRD-02", "TRV-12", "TRV-13", "MET-12"])

    def test_a_named_id_missing_from_the_catalogue_is_an_error(self):
        c = [e for e in self.catalogue() if e["id"] != "MET-12"]
        with self.assertRaises(KeyError):
            fc.rows(c)

    def test_a_named_id_withdrawn_is_an_error(self):
        c = self.catalogue()
        c[2] = entry("TRV-12", "withdrawn")
        with self.assertRaises(KeyError):
            fc.rows(c)

    def test_render_without_a_run_leaves_the_blank(self):
        text = fc.render(self.catalogue())
        self.assertTrue(text.startswith("# Field-test checklist\n"))
        self.assertIn("Run: ________", text)


if __name__ == "__main__":
    unittest.main()

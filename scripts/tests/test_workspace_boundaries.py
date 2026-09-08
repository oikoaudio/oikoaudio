"""Dependency-policy behavior for newly added workspace members."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_workspace import check_dependency_boundaries, workspace_roles


class WorkspaceBoundaries(unittest.TestCase):
    def test_unclassified_member_is_rejected(self):
        with self.assertRaises(SystemExit):
            workspace_roles({"new-engine": {"metadata": None}})

    def test_new_core_cannot_hide_transitive_gui_dependency(self):
        roles = {"new-engine": {"role": "core"}}
        with self.assertRaises(SystemExit):
            check_dependency_boundaries(roles, {"new-engine": "new-engine v1\nhelper v1\negui v1\n"})

    def test_roles_apply_to_arbitrary_new_workspace_names(self):
        roles = {"engine": {"role": "core"}, "widgets": {"role": "ui"}}
        with self.assertRaises(SystemExit):
            check_dependency_boundaries(roles, {"engine": "engine v1\nwidgets v1", "widgets": "widgets v1"})

    def test_product_core_is_isolated_from_other_products_and_shared_code(self):
        for source in ({"role": "core"}, {"role": "plugin", "product": "second"}):
            with self.subTest(source=source), self.assertRaises(SystemExit):
                check_dependency_boundaries(
                    {"consumer": source, "engine": {"role": "core", "product": "first"}},
                    {"consumer": "consumer v1\nengine v1", "engine": "engine v1"},
                )

    def test_transitive_integration_requires_each_consumers_opt_in(self):
        roles = {
            "plugin": {"role": "plugin", "product": "new"},
            "engine": {"role": "core", "product": "new", "integrations": ["parser"]},
            "parser": {"role": "integration"},
        }
        trees = {"plugin": "plugin v1\nengine v1\nparser v1", "engine": "engine v1\nparser v1", "parser": "parser v1"}
        with self.assertRaises(SystemExit):
            check_dependency_boundaries(roles, trees)
        roles["plugin"]["integrations"] = ["parser"]
        check_dependency_boundaries(roles, trees)

    def test_ui_cannot_acquire_a_host_adapter(self):
        with self.assertRaises(SystemExit):
            check_dependency_boundaries({"widgets": {"role": "ui"}}, {"widgets": "widgets v1\negui v1\nbaseview v1"})

    def test_unknown_integration_is_rejected(self):
        with self.assertRaises(SystemExit):
            workspace_roles({"engine": {"metadata": {"oiko": {"role": "core", "integrations": ["missing"]}}}})


if __name__ == "__main__":
    unittest.main()

"""Tests for select()/users_of()/bin_modules() from scripts/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import unittest

from changed_tests_fixtures import _sources, changed_tests


class SelectLibAndBinModules(unittest.TestCase):
    """select() — a path inside core/, an ordinary lib module and its users."""

    def test_use_line_in_test_selects_lib_module_and_its_integration_test(self):
        src = _sources(tests={"identity_restart": "use meshtown::{node, words};\n"})

        sel = changed_tests.select(["core/words.rs"], src)

        self.assertEqual(sel.lib_modules, {"words"})
        self.assertEqual(sel.integration, {"identity_restart"})
        self.assertEqual(sel.bin_modules, set())
        self.assertFalse(sel.web_check)

    def test_directory_module_and_its_tests_file_select_same_top_module(self):
        src = _sources()

        for path in (
            "core/config_file/save.rs",
            "core/config_file/save_tests.rs",
        ):
            with self.subTest(path=path):
                sel = changed_tests.select([path], src)
                self.assertEqual(sel.lib_modules, {"config_file"})

    def test_web_mod_rs_selects_web_module_and_web_check(self):
        sel = changed_tests.select(["core/web/mod.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"web"})
        self.assertTrue(sel.web_check)

    def test_lib_module_used_by_a_binary_and_by_an_example(self):
        src = _sources(
            bin_files={
                "app": "use meshtown::words;\n",
                "main": "use meshtown::words;\n",
            },
            examples={"web_demo": "use meshtown::words;\n"},
        )

        sel = changed_tests.select(["core/words.rs"], src)

        self.assertEqual(sel.lib_modules, {"words"})
        self.assertEqual(sel.bin_modules, {"app"}, "main is a special case, goes to whole_bin")
        self.assertTrue(sel.whole_bin)
        self.assertTrue(sel.web_check)

    def test_binary_module_file_selects_its_own_bin_module_without_lib_or_integration(self):
        src = _sources(main_rs="mod app;\n")

        sel = changed_tests.select(["core/app/view.rs"], src)

        self.assertEqual(sel.bin_modules, {"app"})
        self.assertEqual(sel.lib_modules, set())
        self.assertEqual(sel.integration, set())

    def test_main_rs_and_main_tests_rs_select_whole_bin_not_lib_module(self):
        for path in ("core/main.rs", "core/main_tests.rs"):
            with self.subTest(path=path):
                sel = changed_tests.select([path], _sources())
                self.assertTrue(sel.whole_bin)
                self.assertEqual(sel.lib_modules, set(), "not lib_modules={main_tests}")

    def test_lib_rs_selects_full(self):
        sel = changed_tests.select(["core/lib.rs"], _sources())

        self.assertTrue(sel.full)


class SelectIntegrationTests(unittest.TestCase):
    """select() — direct and indirect (via common/) integration tests."""

    def test_tests_file_selects_its_own_name(self):
        src = _sources(tests={"loopback": "fn it_works() {}\n"})

        sel = changed_tests.select(["core/tests/loopback.rs"], src)

        self.assertEqual(sel.integration, {"loopback"})
        self.assertEqual(sel.lib_modules, set())

    def test_common_file_selects_every_test_that_includes_it(self):
        src = _sources(
            tests={
                "node_web_bootstrap": (
                    '#[path = "common/node_web.rs"]\nmod common_node_web;\n'
                    "use meshtown::node;\n"
                ),
                "loopback": "fn it_works() {}\n",
            }
        )

        sel = changed_tests.select(["core/tests/common/node_web.rs"], src)

        self.assertEqual(sel.integration, {"node_web_bootstrap"})
        self.assertEqual(sel.lib_modules, set())

    def test_lib_module_used_inside_included_common_file_selects_that_integration_test(self):
        src = _sources(
            tests={
                "node_web_bootstrap": (
                    '#[path = "common/node_web.rs"]\nmod common_node_web;\n'
                    "use meshtown::node;\n"
                ),
            }
        )

        sel = changed_tests.select(["core/node/x.rs"], src)

        self.assertEqual(sel.integration, {"node_web_bootstrap"})

    def test_deleted_tests_file_not_in_sources_is_dropped_from_integration(self):
        src = _sources(tests={})

        sel = changed_tests.select(["core/tests/removed_test.rs"], src)

        self.assertEqual(sel.integration, set())
        self.assertEqual(sel.lib_modules, set())


class SelectWebAndScripts(unittest.TestCase):
    """select() — core/web_gui/ui/, core/examples/ and core/web_gui/e2e/ give web_check; core/web_gui/ui/vue/ and core/web_gui/ui/bundle/ additionally js (core/web_gui/ui/vue/ also web_typecheck), e2e also web_lint."""

    def test_js_file_under_web_selects_web_check_js_and_lib_modules(self):
        sel = changed_tests.select(["core/web_gui/ui/vue/src/shell/status-line.ts"], _sources())

        self.assertTrue(sel.web_check)
        self.assertTrue(sel.js)
        self.assertTrue(sel.web_typecheck)
        self.assertEqual(sel.lib_modules, {"web", "node::identity"})

    def test_bundle_file_selects_js_but_not_web_typecheck(self):
        sel = changed_tests.select(["core/web_gui/ui/bundle/app.js"], _sources())

        self.assertTrue(sel.js)
        self.assertFalse(sel.web_typecheck)

    def test_js_file_under_web_does_not_select_web_lint(self):
        sel = changed_tests.select(["core/web_gui/ui/vue/src/shell/status-line.ts"], _sources())

        self.assertFalse(sel.web_lint)

    def test_web_demo_example_selects_web_check_not_py(self):
        sel = changed_tests.select(["core/examples/web_demo/main.rs"], _sources())

        self.assertTrue(sel.web_check)
        self.assertFalse(sel.py)

    def test_e2e_spec_selects_web_check_and_web_lint_only(self):
        sel = changed_tests.select(["core/web_gui/e2e/specs/basic.spec.ts"], _sources())

        self.assertTrue(sel.web_check)
        self.assertTrue(sel.web_lint)
        self.assertEqual(sel.lib_modules, set())
        self.assertFalse(sel.full)
        self.assertFalse(sel.py)

    def test_e2e_package_json_selects_web_check_and_web_lint(self):
        sel = changed_tests.select(["core/web_gui/e2e/package.json"], _sources())

        self.assertTrue(sel.web_check)
        self.assertTrue(sel.web_lint)

    def test_e2e_readme_selects_nothing(self):
        sel = changed_tests.select(["core/web_gui/e2e/README.md"], _sources())

        self.assertEqual(
            (sel.full, sel.web_check, sel.web_lint, sel.py, sel.js),
            (False, False, False, False, False),
        )
        self.assertEqual(sel.reasons, ["core/web_gui/e2e/README.md → no-op (md)"])

    def test_android_source_selects_android_tests_only(self):
        sel = changed_tests.select(["platforms/android/gui/app/src/main/java/A.java"], _sources())

        self.assertTrue(sel.android)
        self.assertEqual(
            (sel.full, sel.py, sel.js, sel.web_check), (False, False, False, False)
        )
        self.assertEqual(sel.reasons, ["platforms/android/gui/app/src/main/java/A.java → android"])

    def test_android_readme_selects_nothing(self):
        sel = changed_tests.select(["platforms/android/gui/README.md"], _sources())

        self.assertFalse(sel.android)


class SelectFullAndEmpty(unittest.TestCase):
    """select() — build files give full, documentation and foreign trees give nothing."""

    def test_build_config_files_select_full_py_js(self):
        for path in (
            "Makefile",
            "Cargo.toml",
            "core/Cargo.toml",
            "src/node/x.rs",
            ".cargo/config.toml",
            "foo.txt",
        ):
            with self.subTest(path=path):
                sel = changed_tests.select([path], _sources())
                self.assertTrue(sel.full, path)
                self.assertTrue(sel.py, path)
                self.assertTrue(sel.js, path)

    def test_docs_and_task_paths_select_nothing(self):
        for path in (
            "README.md",
            "docs/x.md",
            "notes/old/plan.md",
        ):
            with self.subTest(path=path):
                sel = changed_tests.select([path], _sources())
                self.assertEqual(
                    (sel.full, sel.whole_bin, sel.web_check, sel.py, sel.js),
                    (False, False, False, False, False),
                    path,
                )
                self.assertEqual(
                    sel.lib_modules | sel.bin_modules | sel.integration, set(), path
                )

    def test_multiple_paths_union_their_selections(self):
        sel = changed_tests.select(
            ["core/words.rs", "core/web_gui/e2e/specs/basic.spec.ts"],
            _sources(),
        )

        self.assertEqual(sel.lib_modules, {"words"})
        self.assertTrue(sel.web_check)
        self.assertFalse(sel.py)


class UsersOf(unittest.TestCase):
    """users_of() — searching for a whole module word in use expressions, not substrings."""

    def test_finds_qualified_path_use(self):
        found = changed_tests.users_of("node", {"a": "use meshtown::node::start;\n"})

        self.assertEqual(found, {"a"})

    def test_does_not_match_longer_module_name_as_substring(self):
        found = changed_tests.users_of("node", {"a": "use meshtown::node_ctl;\n"})

        self.assertEqual(found, set())

    def test_ignores_mention_inside_line_comment(self):
        found = changed_tests.users_of("node", {"a": "// use meshtown::node;\n"})

        self.assertEqual(found, set())

    def test_finds_module_inside_multiline_use_group(self):
        text = "use meshtown::{\n    node,\n    words,\n};\n"

        found = changed_tests.users_of("node", {"a": text})

        self.assertEqual(found, {"a"})


class BinModules(unittest.TestCase):
    """bin_modules() — all top-level `mod x;` of main.rs, including #[cfg] and pub."""

    def test_collects_all_mod_declarations_including_cfg_and_pub(self):
        main_rs = 'mod app;\n#[cfg(target_os = "macos")]\nmod tray;\npub mod cli;\n'

        self.assertEqual(changed_tests.bin_modules(main_rs), {"app", "tray", "cli"})


if __name__ == "__main__":
    unittest.main()

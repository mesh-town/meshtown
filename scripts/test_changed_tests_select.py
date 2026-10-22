"""Tests for select()/users_of() from scripts/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import os
import unittest
from unittest import mock

from changed_tests_fixtures import _sources, changed_tests


class SelectLibModules(unittest.TestCase):
    """select() — a path inside core/, a library module and its users."""

    def test_use_line_in_test_selects_lib_module_and_its_integration_test(self):
        src = _sources(tests={"identity_restart": "use meshtown_core::{node, members::words};\n"})

        sel = changed_tests.select(["core/members/words.rs"], src)

        self.assertEqual(sel.lib_modules, {"members::words"})
        self.assertEqual(sel.integration, {"identity_restart"})
        self.assertFalse(sel.web_check)

    def test_nested_net_file_selects_transport_net(self):
        sel = changed_tests.select(["core/transport/net/x.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"transport::net"})
        self.assertEqual(sel.reasons, ["core/transport/net/x.rs → lib_modules += transport::net"])

    def test_directory_module_and_its_tests_file_select_same_module(self):
        for path in (
            "core/config/config_file/save.rs",
            "core/config/config_file/save_tests.rs",
            "core/config/config_file_tests.rs",
        ):
            with self.subTest(path=path):
                sel = changed_tests.select([path], _sources())
                self.assertEqual(sel.lib_modules, {"config::config_file"})

    def test_deeper_directories_collapse_to_group_and_first_directory(self):
        self.assertEqual(
            changed_tests.select(["core/node/start/x.rs"], _sources()).lib_modules, {"node::start"}
        )
        self.assertEqual(
            changed_tests.select(["core/node/telemetry.rs"], _sources()).lib_modules,
            {"node::telemetry"},
        )

    def test_group_mod_rs_selects_whole_group(self):
        sel = changed_tests.select(["core/transport/mod.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"transport"})

    def test_top_level_module_file_selects_that_module(self):
        sel = changed_tests.select(["core/invite.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"invite"})

    def test_web_gui_mod_rs_selects_web_gui_module_and_web_check(self):
        sel = changed_tests.select(["core/web_gui/mod.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"web_gui"})
        self.assertTrue(sel.web_check)

    def test_web_gui_rust_subdirectory_is_a_module(self):
        sel = changed_tests.select(["core/web_gui/auth/x.rs"], _sources())

        self.assertEqual(sel.lib_modules, {"web_gui::auth"})

    def test_lib_module_used_by_the_binary_and_by_an_example(self):
        src = _sources(
            main_rs="use meshtown_core::members::words;\n",
            examples={"web_demo": "use meshtown_core::members::words;\n"},
        )

        sel = changed_tests.select(["core/members/words.rs"], src)

        self.assertEqual(sel.lib_modules, {"members::words"})
        self.assertTrue(sel.whole_bin)
        self.assertTrue(sel.web_check)

    def test_cli_main_rs_is_the_only_path_selecting_the_binary(self):
        sel = changed_tests.select(["core/cli/main.rs"], _sources())

        self.assertTrue(sel.whole_bin)
        self.assertEqual(sel.lib_modules, set())
        self.assertEqual(sel.reasons, ["core/cli/main.rs → whole_bin"])

    def test_former_binary_modules_go_through_lib(self):
        for path, module in (
            ("core/cli/run.rs", "cli::run"),
            ("core/cli/run_tests.rs", "cli::run"),
            ("core/gui/tray/app/view.rs", "gui::tray"),
        ):
            with self.subTest(path=path):
                sel = changed_tests.select([path], _sources())
                self.assertFalse(sel.whole_bin)
                self.assertEqual(sel.lib_modules, {module})

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
                    "use meshtown_core::node;\n"
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
                    "use meshtown_core::node;\n"
                ),
            }
        )

        sel = changed_tests.select(["core/node/start/x.rs"], src)

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
        self.assertEqual(sel.lib_modules, {"web_gui", "node::identity"})

    def test_ui_index_html_selects_web_check_and_web_modules_only(self):
        sel = changed_tests.select(["core/web_gui/ui/index.html"], _sources())

        self.assertTrue(sel.web_check)
        self.assertFalse(sel.js)
        self.assertEqual(sel.lib_modules, {"web_gui", "node::identity"})
        self.assertEqual(sel.reasons, ["core/web_gui/ui/index.html → web"])

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
            "core/build.rs.in",
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
                    sel.lib_modules | sel.integration, set(), path
                )

    def test_multiple_paths_union_their_selections(self):
        sel = changed_tests.select(
            ["core/members/words.rs", "core/web_gui/e2e/specs/basic.spec.ts"],
            _sources(),
        )

        self.assertEqual(sel.lib_modules, {"members::words"})
        self.assertTrue(sel.web_check)
        self.assertFalse(sel.py)


class UsersOf(unittest.TestCase):
    """users_of() — searching for a whole module path in use expressions, not substrings."""

    def test_finds_qualified_path_use(self):
        found = changed_tests.users_of("node", {"a": "use meshtown_core::node::start;\n"})

        self.assertEqual(found, {"a"})

    def test_finds_nested_module_path(self):
        found = changed_tests.users_of("transport::net", {"a": "use meshtown_core::transport::net::Conn;\n"})

        self.assertEqual(found, {"a"})

    def test_parent_module_import_counts_as_user_of_child(self):
        found = changed_tests.users_of("node::identity", {"a": "use meshtown_core::node;\n"})

        self.assertEqual(found, {"a"})

    def test_does_not_match_longer_module_name_as_substring(self):
        found = changed_tests.users_of(
            "transport::net", {"a": "use meshtown_core::transport::net_hyper::X;\n"}
        )

        self.assertEqual(found, set())

    def test_does_not_match_sibling_module(self):
        found = changed_tests.users_of(
            "members::words", {"a": "use meshtown_core::members::access::X;\n"}
        )

        self.assertEqual(found, set())

    def test_ignores_mention_inside_line_comment(self):
        found = changed_tests.users_of("node", {"a": "// use meshtown_core::node;\n"})

        self.assertEqual(found, set())

    def test_finds_module_inside_multiline_use_group(self):
        text = "use meshtown_core::{\n    node,\n    members,\n};\n"

        found = changed_tests.users_of("node", {"a": text})

        self.assertEqual(found, {"a"})

    def test_finds_nested_module_inside_group_with_prefix(self):
        text = "use meshtown_core::members::{access, words};\n"

        self.assertEqual(changed_tests.users_of("members::words", {"a": text}), {"a"})
        self.assertEqual(changed_tests.users_of("transport::net", {"a": text}), set())


if __name__ == "__main__":
    unittest.main()


class SelectPlatformCrates(unittest.TestCase):
    """select() — platform crates: the host OS runs everything, a foreign OS nothing, build files fall to the catch-all."""

    def test_host_platform_gui_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/macos/gui/backend.rs"], _sources())

        self.assertTrue(sel.full)

    def test_host_platform_manifest_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "linux"}):
            sel = changed_tests.select(["platforms/linux/Cargo.toml"], _sources())

        self.assertTrue(sel.full)

    def test_foreign_platform_gui_selects_nothing(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/windows/gui/backend.rs"], _sources())

        self.assertEqual(
            (sel.full, sel.py, sel.js, sel.web_check, sel.android), (False, False, False, False, False)
        )

    def test_platform_build_script_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/linux/build/build.sh"], _sources())

        self.assertEqual((sel.full, sel.py, sel.js), (True, True, True))

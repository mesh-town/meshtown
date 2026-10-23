.PHONY: help build build_macos build_macos_app_dev build_linux build_windows build_all test test-cargo test-one test-py test-dist test-js test-page test-vue test-changed android-test clippy clippy-full core-check features-check net-boundary boundary check web-build web-bundle-check web-demo web-check web-check-node web-check-nobuild web-check-node-nobuild web-check-lint web-typecheck web-deps battery

# One cargo build per host at a time — make -j does not break that.
.NOTPARALLEL:

# Project commands — the same as in README.md, section "Building" (and docs/USAGE.md for running the node).
# The build directory is .tmp/build/ at the tree root (in .gitignore); override it with:
# make test TARGET_DIR=/other/dir
BUILD_ROOT ?= $(CURDIR)/.tmp/build
TARGET_DIR ?= $(BUILD_ROOT)/host
# Dynamic linking for dev/test: tests and target binaries pull code from libmeshtown_core.dylib
# and libstd.dylib instead of copying it into each of ~40 executables — the "edit lib → build tests" loop is ~40 % shorter.
# macOS only (Linux/Windows untested) and only for calls through make: the release scripts build/*/build.sh
# and bare cargo stay static. DYN=0 turns it off.
DYN ?= 1
# Feature flags of the node layer without tray (second clippy-full pass, web-check*): the core builds without them too — see core-check.
NODE_FEATURES := net-hyper
DYN_ENV :=
# A RUSTFLAGS with prefer-dynamic already set by the parent (battery -> make test) is not duplicated: otherwise the flag string
# would differ and cargo would rebuild the directory on every "direct make / nested make" switch.
ifeq ($(shell uname -s)$(DYN)$(findstring prefer-dynamic,$(RUSTFLAGS)),Darwin1)
DYN_SYSROOT_LIB := $(shell rustc --print sysroot)/lib/rustlib/$(shell rustc -vV | sed -n 's/^host: //p')/lib
DYN_ENV := RUSTFLAGS="$(RUSTFLAGS) -C prefer-dynamic -C rpath -C link-arg=-Wl,-rpath,$(DYN_SYSROOT_LIB)"
endif
CARGO := CARGO_TARGET_DIR=$(TARGET_DIR) $(DYN_ENV) cargo
WEB_PORT ?= 7777
WEB_TOKEN ?= dev
# make test-changed — the comparison base (merge-base with the working tree) and a dry run without executing.
BASE ?= main
# macOS .app bundle (platforms/macos/build/app.sh): APP_NAME is the bundle file name (MeshTown.app, no
# space), APP_TITLE is the human-readable name in Info.plist.
APP_NAME ?= MeshTown
APP_TITLE ?= Mesh Town
BUNDLE_ID ?= local.meshtown
# Result directory of the e2e battery: one per tree, so worktrees do not write into the same .tmp/e2e
# (see core/web_gui/e2e/fixtures/orphans.ts); a value from the environment or the command line wins.
export E2E_RUN_TAG ?= $(notdir $(CURDIR))
export BUILD_ROOT TARGET_DIR APP_NAME APP_TITLE BUNDLE_ID

# CONTAINER is export only, with no default here: the build/ scripts choose container/docker themselves
# by OS (scripts/release/lib.sh). A `?= container` in this Makefile would override that choice — an empty
# export does not override the script default, but `?= container` with a non-empty value would.
export CONTAINER

# Host OS: make build builds for it.
ifeq ($(OS),Windows_NT)
HOST_OS := windows
else
HOST_OS := $(shell uname -s | tr A-Z a-z)
endif
ifeq ($(HOST_OS),darwin)
BUILD_HOST_OS := macos
else
BUILD_HOST_OS := $(HOST_OS)
endif

# Contents of build_<os> — one place per system:
# changing the contents means editing one line; the build_<os> prerequisites run in this order.
MACOS_KINDS := cli app
LINUX_KINDS := cli gui
WINDOWS_KINDS := cli gui

help:
	@echo "make build                      — release build of meshtown for this computer's OS ($(HOST_OS))"
	@echo "make build_macos                — $(MACOS_KINDS) for macOS (Mac only)"
	@echo "make build_linux                — $(LINUX_KINDS) for Linux x86_64 (container)"
	@echo "make build_windows              — $(WINDOWS_KINDS) for Windows x86_64 (container)"
	@echo "make build_all                  — build_macos (Mac only) + build_linux + build_windows"
	@echo "make build_macos_cli            — macOS binary without tray (--no-default-features --features $(NODE_FEATURES)), no bundle: dist/meshtown-macos-cli-<version>"
	@echo "make build_macos_app            — .app with tray, arm64: dist/macos-arm64/MeshTown-<version>.app"
	@echo "make build_macos_app_dev      — debug .app (arm64, no --release, for look probes): dist/macos-arm64/Mesh-<version>-dev.app"
	@echo "make build_macos_app_x86        — .app with tray, x86_64: dist/macos-x86_64/MeshTown-<version>.app"
	@echo "make build_macos_app_universal  — .app with tray, universal (lipo arm64+x86_64): dist/macos-universal/MeshTown-<version>.app"
	@echo "make build_linux_cli            — Linux binary without tray: dist/meshtown-linux-cli-<version>"
	@echo "make build_linux_gui            — Linux binary with tray: dist/meshtown-linux-gui-<version>"
	@echo "make build_linux_arm64_cli      — Linux arm64 binary (OpenWrt router) without tray: dist/meshtown-linux-arm64-cli-<version>"
	@echo "make build_linux_armv7_cli      — Linux ARMv7 binary (OpenWrt router, soft-float) without tray: dist/meshtown-linux-armv7-cli-<version>"
	@echo "make build_windows_cli          — Windows binary without tray: dist/meshtown-windows-cli-<version>.exe"
	@echo "make build_windows_gui          — Windows binary with tray: dist/meshtown-windows-gui-<version>.exe"
	@echo "make <os>-build|test|run|package — commands of the platform crate (os: macos, linux, windows); package takes KIND=<kind>, run takes ARGS=<args>"
	@echo "make test                       — cargo nextest run (no nextest — cargo test)"
	@echo "make test-cargo                 — cargo test (old path, no nextest)"
	@echo "make test-one T=<name> [TARGET_DIR=…] — one Rust test with the same flags and cache as make test"
	@echo "make test-py                    — unittest over scripts/{battery,changed_tests,checks}/test_*.py and platforms/linux/build/openwrt/feed/test_*.py and platforms/linux/build/keenetic/feed/test_*.py"
	@echo "make test-js                    — vitest: *.test.mjs logic (node environment) and *.vtest.mjs components (happy-dom)"
	@echo "make test-vue                   — components only (vitest --project dom): core/web_gui/ui/vue/src/**/*.vtest.mjs"
	@echo "make web-build                  — build the page (Vite): core/web_gui/ui/vue → core/web_gui/ui/bundle/app.js (committed)"
	@echo "make web-bundle-check           — bundle/app.js is fresh: a build into a temp directory matches the commit"
	@echo "make test-page                  — before committing a page stage: test-js + Rust tests that read core/web_gui/ui/vue and core/web_gui/ui/bundle (identity_texts_match_page, bundle_app_js)"
	@echo "make android-test                — JVM unit tests of the Android shell (platforms/android/gui/test.sh)"
	@echo "make test-changed               — tests affected by the change relative to BASE (main by default); DRY=1 — list only"
	@echo "make clippy                     — fast loop: clippy over all targets, default feature set only, warnings = errors"
	@echo "make clippy-full                — acceptance: clippy in two passes (default and --no-default-features --features $(NODE_FEATURES), i.e. without tray)"
	@echo "make core-check                — core without the node layer: clippy and tests --lib --no-default-features (cargo test --no-default-features without --lib is unsupported — it needs the node)"
	@echo "make features-check             — node without tray (--no-default-features --features net-hyper): clippy and net_names/gate tests; run when editing cfg"
	@echo "make boundary                   — core/platform boundary: manifests, include paths and platform scripts stay on their side"
	@echo "make check                      — test + test-py + test-js + web-typecheck + web-bundle-check + clippy-full + core-check + net-boundary + boundary"
	@echo "make web-demo                   — demo node for the page (fake data, no real node build): http://127.0.0.1:$(WEB_PORT)/?token=$(WEB_TOKEN)"
	@echo "make web-check                  — Playwright battery (e2e) on web_demo, builds without tray itself (ONLY=\"14 18\" — case numbers or names separated by spaces; JOBS=<N> — workers, 4 by default for demo, 1 for node; HEADED=1 — browser window)"
	@echo "make web-check-node             — the same battery on a real meshtown node (builds without tray itself)"
	@echo "make web-check-nobuild          — web-check without cargo: a ready binary E2E_DEMO_BIN (from TARGET_DIR by default)"
	@echo "make web-check-node-nobuild     — web-check-node without cargo: a ready binary E2E_NODE_BIN (from TARGET_DIR by default)"
	@echo "make web-deps                   — install npm dependencies and chromium for e2e and vue (the battery calls it before the lanes)"
	@echo "make web-check-lint             — typecheck and eslint over e2e"
	@echo "make web-typecheck             — vue-tsc --noEmit over core/web_gui/ui/vue (no cargo)"
	@echo "make battery                    — the whole acceptance battery in one run, markdown report (OUT=<path>, ONLY=<item,item>; SERIAL=1 — the old sequential mode, no build phases or lanes)"
	@echo "DYN=0                           — make test/clippy/battery without dynamic linking (on by default on macOS)"

build: build_$(BUILD_HOST_OS)

# Binaries are built by platforms/<system>/build/build.sh (scripts/release/lib.sh is the shared part); the targets
# here are one-line calls, the logic (feature choice, container, lipo, packaging) lives only in the scripts.
build_macos: $(addprefix build_macos_,$(MACOS_KINDS))

build_linux: $(addprefix build_linux_,$(LINUX_KINDS))

build_windows: $(addprefix build_windows_,$(WINDOWS_KINDS))

# Pattern targets are not in .PHONY: GNU make does not look for a pattern for phony targets, and no files
# with such names exist or ever will.
# Debug .app for manual checks: no --release, the name in dist/ has a -dev suffix. The explicit target
# overrides the build_macos_% pattern.
build_macos_app_dev:
	$(MAKE) -C platforms/macos package KIND=app PROFILE=dev

build_macos_%:
	$(MAKE) -C platforms/macos package KIND=$*

build_linux_%:
	$(MAKE) -C platforms/linux package KIND=$*

build_windows_%:
	$(MAKE) -C platforms/windows package KIND=$*

# Platform crate commands: `make macos-build`, `make linux-test`, `make windows-package KIND=cli`, ...
macos-%:
	$(MAKE) -C platforms/macos $*

linux-%:
	$(MAKE) -C platforms/linux $*

windows-%:
	$(MAKE) -C platforms/windows $*

ifeq ($(HOST_OS),darwin)
build_all: build_macos build_linux build_windows
else
build_all: build_linux build_windows
endif

test:
	@if ($(CARGO) nextest --version >/dev/null 2>&1); then $(CARGO) nextest run --workspace; else $(CARGO) test --workspace; fi

# One Rust test with the same RUSTFLAGS, CARGO_TARGET_DIR and runner as `make test` (the shared cache stays intact).
test-one:
	@test -n "$(T)" || { echo "make test-one: give the full test name: make test-one T=module::test [TARGET_DIR=…]"; exit 2; }
	@$(CARGO) nextest --version >/dev/null 2>&1 || { echo "make test-one: cargo nextest is required"; exit 2; }
	$(CARGO) nextest run --workspace -E 'test(=$(T))'

test-cargo:
	$(CARGO) test --workspace

test-py:
	python3 -m unittest discover -s scripts/battery -t scripts/battery -p 'test_*.py'
	python3 -m unittest discover -s scripts/changed_tests -t scripts/changed_tests -p 'test_*.py'
	python3 -m unittest discover -s scripts/checks -t scripts/checks -p 'test_*.py'
	python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed -p 'test_*.py'
	python3 -m unittest discover -s platforms/linux/build/keenetic/feed -t platforms/linux/build/keenetic/feed -p 'test_*.py'

# The publishing script of the distribution repository, against a local bare repository.
test-dist:
	scripts/release/dist/publish_test.sh
	bash scripts/release/lib_test.sh

UI := core/web_gui/ui
VUE := $(UI)/vue
BUNDLE := $(UI)/bundle
VUE_DEPS := $(VUE)/node_modules/.package-lock.json
BUNDLE_TMP := $(CURDIR)/.tmp/web-bundle
$(VUE_DEPS): $(VUE)/package-lock.json
	cd $(VUE) && npm ci

# Page logic and components in one runner: vitest, projects node (*.test.mjs) and dom (*.vtest.mjs).
test-js: $(VUE_DEPS)
	cd $(VUE) && npx vitest run

# Before committing a page stage: test-js + Rust tests that read the core/web_gui/ui/vue/src and core/web_gui/ui/bundle files
# (node::identity::tests::identity_texts_match_page, web::routes::pages::tests::bundle_app_js::*).
test-page: test-js
	$(CARGO) nextest run -E 'test(identity_texts_match_page) | test(bundle_app_js)'

# Page components only (fast loop over .vue): vitest --project dom + @vue/test-utils + happy-dom,
# files core/web_gui/ui/vue/src/**/*.vtest.mjs.
test-vue: $(VUE_DEPS)
	cd $(VUE) && npx vitest run --project dom

# The page is built with Vue: sources core/web_gui/ui/vue/src → a single file core/web_gui/ui/bundle/app.js (in git,
# so cargo build does not require Node). The bundle/ directory is never edited by hand — only by this target.
web-build: $(VUE_DEPS)
	cd $(VUE) && npx vite build --outDir $(CURDIR)/$(BUNDLE) --emptyOutDir

# Build freshness: build into a temp directory and compare byte for byte with the committed file.
web-bundle-check: $(VUE_DEPS)
	rm -rf $(BUNDLE_TMP)
	cd $(VUE) && npx vite build --outDir $(BUNDLE_TMP) --emptyOutDir --logLevel warn
	@test "$$(ls $(BUNDLE_TMP))" = app.js || { echo "$(BUNDLE_TMP) must contain exactly app.js"; exit 1; }
	@cmp $(BUNDLE_TMP)/app.js $(BUNDLE)/app.js || { echo "bundle/app.js is stale: make web-build"; exit 1; }
	@test "$$(ls $(BUNDLE))" = app.js || { echo "$(BUNDLE) must contain exactly app.js"; exit 1; }

test-changed:
	CARGO_TARGET_DIR=$(TARGET_DIR) $(DYN_ENV) python3 scripts/changed_tests/changed_tests.py --base $(BASE) $(if $(DRY),--dry-run)

# JVM unit tests of the Android shell (no emulator, NDK or Rust needed; needs the Android SDK and JDK 17+).
android-test:
	bash platforms/android/gui/test.sh

# Fast loop — one pass (default); acceptance (check, battery) — clippy-full, both feature sets.
clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

clippy-full: clippy
	$(CARGO) clippy --all-targets --no-default-features --features $(NODE_FEATURES) -- -D warnings

# Core without the node layer and foreign network crates: --lib only (the binary and the core/tests/ directory tests require net-hyper).
core-check:
	$(CARGO) clippy --lib --no-default-features -- -D warnings
	$(CARGO) test --lib --no-default-features

# Node without tray: a set of a single feature net-hyper (--no-default-features); catches cfg that depends on default.
# Not part of `check`: run when editing cfg.
features-check:
	$(CARGO) clippy --all-targets --no-default-features --features $(NODE_FEATURES) -- -D warnings
	$(CARGO) test --lib --no-default-features --features $(NODE_FEATURES) -- net_names gate

# -k: a red target does not abort the rest — the battery shows all red ones in one go.
check:
	$(MAKE) -k test test-py test-dist test-js web-typecheck web-bundle-check clippy-full core-check net-boundary boundary

# Network core boundary: core/transport/net has no foreign network crates and no paths outside the core;
# no path within the core boundary names foreign network crates; townjump crates live only in net_hyper/.
# H4: townjump crates (townjump_*, peeroxide_*, libudx) live only in net_hyper/; `net_hyper::` outside net_hyper/ only in node/start/.
# H5: net_hyperdht does not name net_hyper, mainline, townjump or peeroxide — only the RecordStore trait and common types.
net-boundary:
	@test -d core/transport/net || { echo "net-boundary: core/transport/net is missing" >&2; exit 1; }
	@! rg -n -g '!*_tests.rs' -e '\b(mainline|rustls|tokio_rustls|simple_dns|n0_future|axum|hyper\w*|tower\w*|url)::' -e '\bcrate::(\{|\w+)' -e '\bsuper::super\b' core/transport/net | rg -v 'crate::transport::net\b'
	@test -d core/transport/net_hyperdht || { echo "net-boundary: core/transport/net_hyperdht is missing" >&2; exit 1; }
	@! rg -n -g '!*_tests.rs' '\b(net_hyper|mainline|townjump_\w+|peeroxide_\w+)::' core/transport/net_hyperdht
	@test -d core || { echo "net-boundary: core is missing" >&2; exit 1; }
	@! rg -n -e '\b(townjump_\w+|peeroxide_\w+|libudx)::' core -g '!**/net_hyper/**'
	@test -d core || { echo "net-boundary: core is missing" >&2; exit 1; }
	@! rg -n '\bnet_hyper::' core -g '!**/net_hyper/**' -g '!**/node/start/**'

# Core/platform boundary: manifests, include paths and platform scripts stay on their side (net-boundary looks at network crates inside the core).
boundary:
	python3 scripts/checks/check_boundary.py

web-typecheck: $(VUE_DEPS)
	cd $(VUE) && npx vue-tsc --noEmit

web-demo:
	$(CARGO) run --example web_demo -- --port $(WEB_PORT) --token $(WEB_TOKEN)

# The whole acceptance battery in one run (module battery): runs make test/clippy-full/test-js/test-py/
# web-check/web-check-node + cargo --no-default-features in sequence, writes a markdown report as a table and
# the full log of each item to .tmp/runs/battery/. OUT/ONLY are the same names as in the other targets
# (ONLY here means battery.py item names, not web-check cases: GNU make passes ONLY down to children,
# so battery.py calls each sub-make with an explicit `ONLY=` — otherwise the item name would become a Playwright filter).
battery:
	$(DYN_ENV) python3 scripts/battery/battery.py $(if $(OUT),--out $(OUT)) $(if $(ONLY),--only $(ONLY)) $(if $(SERIAL),--serial)

E2E := core/web_gui/e2e
E2E_DEPS := $(E2E)/node_modules/.package-lock.json
E2E_BROWSER := $(E2E)/node_modules/.e2e-chromium
E2E_PW := node_modules/.bin/playwright
E2E_DEMO_BIN ?= $(TARGET_DIR)/debug/examples/web_demo
E2E_NODE_BIN ?= $(TARGET_DIR)/debug/meshtown
E2E_NOTHING :=
E2E_SPACE := $(E2E_NOTHING) $(E2E_NOTHING)
# ONLY="14 18" -> matches only at the start of a case name (number, then -letter)
E2E_GREP = $(if $(strip $(ONLY)),--grep '(^| )(?=[0-9]+-[a-z])($(subst $(E2E_SPACE),|,$(patsubst case-%,%,$(strip $(ONLY)))))([-: ]|$$)')
# Playwright run: $(1) project, $(2) binary environment variable name, $(3) binary path, $(4) default workers
define E2E_RUN
@test -x "$(3)" || { echo "no binary: $(3)"; echo "build it (make web-check / web-check-node) or set $(2)=<path>"; exit 2; }
cd $(E2E) && $(if $(HEADED),WEB_E2E_HEADED=1 )$(2)="$(abspath $(3))" $(E2E_PW) test --project=$(1) --workers=$(or $(JOBS),$(4)) $(E2E_GREP)
endef
$(E2E_DEPS): $(E2E)/package-lock.json
	cd $(E2E) && npm ci
$(E2E_BROWSER): $(E2E_DEPS)
	cd $(E2E) && $(E2E_PW) install chromium && touch node_modules/.e2e-chromium
# The targets build the binary WITHOUT tray (`--no-default-features --features $(NODE_FEATURES)`): the e2e cases do not
# touch the tray — the `run` subcommand is passed explicitly (core/web_gui/e2e/fixtures/node-config.ts, buildNodeArgs), and
# the "bypassing the page" write goes through the HTTP API. A web change needs one feature set instead of
# two: the cases stay green on the thin binary. Side effect: after these targets
# `$(TARGET_DIR)/debug/` holds a binary without tray;
# any default build (`make test`) brings back the binary with tray, seconds with a warm cache.
web-check: $(E2E_BROWSER)
	$(CARGO) build --example web_demo --no-default-features --features $(NODE_FEATURES)
	$(call E2E_RUN,demo,E2E_DEMO_BIN,$(TARGET_DIR)/debug/examples/web_demo,4)
web-check-node: $(E2E_BROWSER)
	$(CARGO) build --bin meshtown --no-default-features --features $(NODE_FEATURES)
	$(call E2E_RUN,node,E2E_NODE_BIN,$(TARGET_DIR)/debug/meshtown,1)
web-check-nobuild: $(E2E_BROWSER)
	$(call E2E_RUN,demo,E2E_DEMO_BIN,$(E2E_DEMO_BIN),4)
web-check-node-nobuild: $(E2E_BROWSER)
	$(call E2E_RUN,node,E2E_NODE_BIN,$(E2E_NODE_BIN),1)
# Dependencies of e2e and vue together: the battery installs them once before the parallel lanes.
web-deps: $(E2E_BROWSER) $(VUE_DEPS)
web-check-lint: $(E2E_DEPS)
	cd $(E2E) && npm run typecheck && npm run lint

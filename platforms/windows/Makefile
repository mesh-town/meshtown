.PHONY: build test run package installer

# Platform commands for the Windows crate (meshtown-windows); run from here or through the root Makefile (`make windows-<target>`).
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../..)
# Package kind for `package`: gui or cli.
KIND ?= gui

build:
	cd $(ROOT) && cargo build -p meshtown-windows

test:
	cd $(ROOT) && cargo nextest run -p meshtown-windows

run:
	cd $(ROOT) && cargo run -p meshtown-windows -- $(ARGS)

package:
	cd $(ROOT) && ./platforms/windows/build/build.sh $(KIND)

# NSIS installer: builds the gui exe, then dist/meshtown-setup-<version>.exe.
installer:
	cd $(ROOT) && ./platforms/windows/build/build.sh gui && ./platforms/windows/build/installer.sh

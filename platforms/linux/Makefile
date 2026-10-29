.PHONY: build test run package tray-e2e

# Platform commands for the Linux crate (meshtown-linux); run from here or through the root Makefile (`make linux-<target>`).
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../..)
# Package kind for `package`: gui or cli.
KIND ?= gui

build:
	cd $(ROOT) && cargo build -p meshtown-linux

test:
	cd $(ROOT) && cargo nextest run -p meshtown-linux

run:
	cd $(ROOT) && cargo run -p meshtown-linux -- $(ARGS)

package:
	cd $(ROOT) && ./platforms/linux/build/build.sh $(KIND)

# Tray icon end-to-end test with a fake StatusNotifierWatcher, in an arm64 container.
tray-e2e:
	$(ROOT)/platforms/linux/tray-e2e.sh

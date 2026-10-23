.PHONY: build test run package

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

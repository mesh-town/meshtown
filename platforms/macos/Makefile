.PHONY: build test run package

# Platform commands for the macOS crate (meshtown-macos); run from here or through the root Makefile (`make macos-<target>`).
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../..)
# Package kind for `package`: app, app_x86, app_universal or cli.
KIND ?= app
BUILD_ROOT ?= $(ROOT)/.tmp/build
TARGET_DIR ?= $(BUILD_ROOT)/host
# Dynamic linking for dev/test, as in the root Makefile; DYN=0 turns it off.
DYN ?= 1
DYN_ENV :=
ifeq ($(shell uname -s)$(DYN)$(findstring prefer-dynamic,$(RUSTFLAGS)),Darwin1)
DYN_SYSROOT_LIB := $(shell rustc --print sysroot)/lib/rustlib/$(shell rustc -vV | sed -n 's/^host: //p')/lib
DYN_ENV := RUSTFLAGS="$(RUSTFLAGS) -C prefer-dynamic -C rpath -C link-arg=-Wl,-rpath,$(DYN_SYSROOT_LIB)"
endif
CARGO := CARGO_TARGET_DIR=$(TARGET_DIR) $(DYN_ENV) cargo

build:
	cd $(ROOT) && $(CARGO) build -p meshtown-macos

test:
	cd $(ROOT) && $(CARGO) nextest run -p meshtown-macos

run:
	cd $(ROOT) && $(CARGO) run -p meshtown-macos -- $(ARGS)

package:
	cd $(ROOT) && ./platforms/macos/build/build.sh $(KIND)

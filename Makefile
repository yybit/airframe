# AirFrame's developer entry points.
#
# Each target is a thin wrapper over the packaging scripts, so every step has
# one implementation. Run `make` on its own to list them.

# Lint and test the way CI does. The UI snapshot goldens were rendered with
# Mesa's software GL rasteriser, so pin that backend on Linux.
ifeq ($(shell uname -s),Linux)
TEST_ENV := WGPU_BACKEND=gl
endif

.PHONY: help run test lint mac mac-arm64 mac-x86_64 mac-universal

help: ## List these targets
	@grep -E '^[a-z0-9_%-]+:.*##' $(MAKEFILE_LIST) \
	    | sed -E 's/:.*##[[:space:]]*/\t/' \
	    | expand -t 18

run: ## Build and launch the desktop app
	cargo run --release

test: ## Run the test suite, including the UI snapshots
	$(TEST_ENV) cargo test --workspace

lint: ## Lint exactly what CI lints
	cargo clippy --workspace --all-targets -- -D warnings

# The three macOS targets need macOS: package.sh builds, signs and archives the
# bundle there. A bundle for the other architecture can be built on Apple
# Silicon with Rosetta's toolchain, or cross-built from Linux (see
# packaging/macos/README.md).
mac: ## Package .app and .zip for this machine (needs macOS)
	packaging/macos/package.sh

mac-arm64: ## Package the Apple Silicon build (needs macOS)
	packaging/macos/package.sh --arch aarch64

mac-x86_64: ## Package the Intel build (needs macOS)
	packaging/macos/package.sh --arch x86_64

mac-universal: ## Package one bundle with both slices (needs macOS)
	packaging/macos/package.sh --arch universal

# Cross-build from Linux, given a macOS SDK: SDKROOT=~/sdk/MacOSX15.5.sdk make
# mac-cross-x86_64
mac-cross-%: ## Cross-build from Linux (mac-cross-x86_64, mac-cross-aarch64)
	@test -n "$$SDKROOT" \
	    || { echo "set SDKROOT to a macOS SDK; see packaging/macos/README.md" >&2; exit 1; }
	packaging/macos/cross-build.sh --target $*-apple-darwin
	packaging/macos/package.sh --arch $* --skip-build \
	    --binary target/$*-apple-darwin/release/airframe

# Desktop Mascots (Rust port) -- common tasks. Run `make` to list them.

ARGS   ?=
WIN    := x86_64-pc-windows-gnu
# zig command for the Windows link when mingw-w64 isn't installed (pip install ziglang -> "python3 -m ziglang")
ZIG    ?= zig
# where `make install` puts the binary: $XDG_BIN_HOME if set, else ~/.local/bin
BINDIR ?= $(or $(XDG_BIN_HOME),$(HOME)/.local/bin)

.DEFAULT_GOAL := help
.PHONY: help run test preview build build-windows install uninstall clean

help: ## List the available targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-14s %s\n", $$1, $$2}'
	@echo
	@echo "  e.g. make run ARGS=\"goose --time 5 --say hi\""

run: ## Start the mascots (pass options with ARGS="...")
	cargo run --release -- $(ARGS)

test: ## Run the headless behaviour tests
	cargo test

preview: ## Render every animation to preview.png
	cargo run --release --example preview -- preview.png

build: ## Build target/release/desktop-mascots for this OS
	cargo build --release

build-windows: ## Cross-compile target/x86_64-pc-windows-gnu/release/desktop-mascots.exe
	rustup target add $(WIN)
	@if command -v x86_64-w64-mingw32-gcc >/dev/null; then \
		cargo build --release --target $(WIN); \
	else \
		echo "mingw-w64 not found: linking with zig ($(ZIG))"; \
		ZIG="$(ZIG)" PATH="$(CURDIR)/tools:$$PATH" \
		CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$(CURDIR)/tools/zig-cc-windows" \
		cargo build --release --target $(WIN); \
	fi
	@ls -l target/$(WIN)/release/desktop-mascots.exe

install: build ## Copy the binary to $XDG_BIN_HOME or ~/.local/bin (override with BINDIR=...)
	install -Dm755 target/release/desktop-mascots "$(BINDIR)/desktop-mascots"
	@echo "installed $(BINDIR)/desktop-mascots"
	@case ":$$PATH:" in *":$(BINDIR):"*) ;; *) echo "note: $(BINDIR) is not on your PATH";; esac

uninstall: ## Remove the installed binary
	rm -f "$(BINDIR)/desktop-mascots"

clean: ## Remove build output and the preview
	cargo clean
	rm -f preview.png

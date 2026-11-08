# Desktop Mascots -- common tasks. Run `make` to list them.

PY     ?= python3
VENV   ?= .venv
ARGS   ?=

ifeq ($(OS),Windows_NT)
BIN := $(VENV)/Scripts
else
BIN := $(VENV)/bin
endif

STAMP := $(VENV)/.installed

.DEFAULT_GOAL := help
.PHONY: help install run say test preview build clean distclean

help: ## List the available targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-10s %s\n", $$1, $$2}'
	@echo
	@echo "  e.g. make run ARGS=\"goose --time 5 --say hi\""
	@echo "       make say ARGS=\"--to goose HONK\""

$(STAMP): pyproject.toml
	$(PY) -m venv $(VENV)
	$(BIN)/python -m pip install -q --upgrade pip
	$(BIN)/python -m pip install -q -e .
	@touch $@

install: $(STAMP) ## Create the venv and install the package (editable)

run: $(STAMP) ## Start the mascots (pass options with ARGS="...")
	$(BIN)/desktop-mascots $(ARGS)

say: $(STAMP) ## Send text to the running mascots (ARGS="[--to MASCOT] TEXT")
	$(BIN)/desktop-mascots say $(ARGS)

test: $(STAMP) ## Run the headless behaviour tests
	$(BIN)/python -m unittest discover tests

preview: $(STAMP) ## Render every animation to preview.png
	$(BIN)/python tools/preview.py preview.png

build: $(STAMP) ## Build a standalone app into dist/ (for the current OS)
	$(BIN)/python -m pip install -q pyinstaller
	$(BIN)/pyinstaller --noconfirm --clean --noconsole --onefile -n DesktopMascots desktop_mascots/__main__.py

clean: ## Remove build output, caches and the preview
	rm -rf build dist *.spec *.egg-info preview.png
	find . -path ./$(VENV) -prune -o -name __pycache__ -type d -exec rm -rf {} +

distclean: clean ## clean, and also delete the venv
	rm -rf $(VENV)

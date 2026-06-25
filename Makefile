.PHONY: help install dev build build-release check lint release clean version-bump

RELEASE_VERSION := $(if $(VERSION),$(VERSION),$(version))
RELEASE_MESSAGE := $(if $(MESSAGE),$(MESSAGE),$(message))

APP_NAME := Plume
TAURI_DIR := src-tauri
CARGO := cargo
NPM := npm

help:
	@echo "Available commands:"
	@echo "  make install                     - Install all dependencies"
	@echo "  make dev                         - Start development server"
	@echo "  make build                       - Build debug bundle"
	@echo "  make build-release               - Build release bundle (NSIS installer)"
	@echo "  make check                       - Run all quality gates (lint + clippy + test)"
	@echo "  make lint                        - Run linters (frontend + backend)"
	@echo "  make version-bump version=x.y.z  - Bump version across all config files"
	@echo "  make release version=x.y.z [message='msg'] - Bump version, commit, tag and push"
	@echo "  make clean                       - Clean build artifacts and caches"

install:
	@echo "Installing frontend dependencies..."
	$(NPM) ci
	@echo "Installing Rust dependencies..."
	$(CARGO) fetch --manifest-path $(TAURI_DIR)/Cargo.toml
	@echo "Installation complete."

dev:
	$(NPM) run tauri dev

build:
	$(NPM) run tauri build

build-release:
	$(NPM) run tauri build -- --release

lint:
	@echo "Checking frontend..."
	$(NPM) run build
	@echo "Checking Rust with clippy..."
	$(CARGO) clippy --manifest-path $(TAURI_DIR)/Cargo.toml -- -D warnings

check: lint
	@echo "Running Rust tests..."
	$(CARGO) test --manifest-path $(TAURI_DIR)/Cargo.toml
	@echo "All quality gates passed."

version-bump:
	@if [ -z "$(RELEASE_VERSION)" ]; then echo "Error: provide version=x.y.z"; exit 1; fi
	@if ! echo "$(RELEASE_VERSION)" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$'; then echo "Error: invalid version format. Expected x.y.z"; exit 1; fi
	@echo "Bumping version to $(RELEASE_VERSION)..."
	@sed -i 's/"version": "[^"]*"/"version": "$(RELEASE_VERSION)"/' $(TAURI_DIR)/tauri.conf.json
	@sed -i 's/^version = "[^"]*"/version = "$(RELEASE_VERSION)"/' $(TAURI_DIR)/Cargo.toml
	@sed -i 's/"version": "[^"]*"/"version": "$(RELEASE_VERSION)"/' package.json
	@echo "$(RELEASE_VERSION)" > version
	@echo "Version bumped to $(RELEASE_VERSION) in tauri.conf.json, Cargo.toml, package.json, version"

release: check version-bump
	@if [ -z "$(RELEASE_VERSION)" ]; then echo "Error: provide version=x.y.z"; exit 1; fi
	MESSAGE_INPUT="$(RELEASE_MESSAGE)"; \
	if [ -z "$$MESSAGE_INPUT" ]; then \
		MESSAGE_INPUT="Release v$(RELEASE_VERSION)"; \
	fi; \
	echo "Staging files..."; \
	git add -A; \
	echo "Creating commit..."; \
	git commit -m "$$MESSAGE_INPUT"; \
	echo "Pushing to origin..."; \
	git push origin HEAD; \
	echo "Creating tag v$(RELEASE_VERSION)..."; \
	git tag -a "v$(RELEASE_VERSION)" -m "$$MESSAGE_INPUT"; \
	echo "Pushing tag..."; \
	git push origin "v$(RELEASE_VERSION)"; \
	echo ""; \
	echo "Release v$(RELEASE_VERSION) created."; \
	echo "GitHub Actions will build and publish automatically."; \
	echo "https://github.com/b7s/plume/releases/tag/v$(RELEASE_VERSION)"

clean:
	@echo "Cleaning frontend artifacts..."
	rm -rf dist/
	rm -rf node_modules/.vite/
	@echo "Cleaning Rust artifacts..."
	rm -rf $(TAURI_DIR)/target/
	@echo "Clean complete."

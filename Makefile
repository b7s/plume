SHELL := C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.3.0_x64__8wekyb3d8bbwe\pwsh.exe
.SHELLFLAGS := -NoLogo -NoProfile -Command

.PHONY: help install dev build build-release check lint release clean version-bump

RELEASE_VERSION := $(if $(VERSION),$(VERSION),$(version))
RELEASE_MESSAGE := $(if $(MESSAGE),$(MESSAGE),$(message))

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
	@echo "  make release version=x.y.z [message='msg'] - Commit, tag, push, then update local files"
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
	@if (-not "$(RELEASE_VERSION)") { Write-Error "provide version=x.y.z"; exit 1 }
	@if ("$(RELEASE_VERSION)" -notmatch '^\d+\.\d+\.\d+$$') { Write-Error "invalid version format. Expected x.y.z"; exit 1 }
	@echo "Bumping version to $(RELEASE_VERSION)..."
	@(Get-Content $(TAURI_DIR)/tauri.conf.json) -replace '"version": "[^"]*"', '"version": "$(RELEASE_VERSION)"' | Set-Content $(TAURI_DIR)/tauri.conf.json
	@(Get-Content $(TAURI_DIR)/Cargo.toml) -replace '^version = "[^"]*"', 'version = "$(RELEASE_VERSION)"' | Set-Content $(TAURI_DIR)/Cargo.toml
	@(Get-Content package.json) -replace '"version": "[^"]*"', '"version": "$(RELEASE_VERSION)"' | Set-Content package.json
	@Set-Content -Path version -Value "$(RELEASE_VERSION)"
	@echo "Version bumped to $(RELEASE_VERSION) in tauri.conf.json, Cargo.toml, package.json, version"

release: check
	@if (-not "$(RELEASE_VERSION)") { Write-Error "provide version=x.y.z"; exit 1 }
	@if ("$(RELEASE_VERSION)" -notmatch '^\d+\.\d+\.\d+$$') { Write-Error "invalid version format. Expected x.y.z"; exit 1 }
	@$$msg = "$(RELEASE_MESSAGE)"; if (-not $$msg) { $$msg = "Release v$(RELEASE_VERSION)" }; echo "Staging files..."; git add -A; echo "Creating commit..."; git commit -m $$msg; echo "Pushing to origin..."; git push origin HEAD; echo "Creating tag v$(RELEASE_VERSION)..."; git tag -a "v$(RELEASE_VERSION)" -m $$msg; echo "Pushing tag..."; git push origin "v$(RELEASE_VERSION)"; echo ""; echo "Tag v$(RELEASE_VERSION) pushed. GitHub Actions will build the release."; echo "Updating local config files to new version..."; (Get-Content $(TAURI_DIR)/tauri.conf.json) -replace '"version": "[^"]*"', '"version": "$(RELEASE_VERSION)"' | Set-Content $(TAURI_DIR)/tauri.conf.json; (Get-Content $(TAURI_DIR)/Cargo.toml) -replace '^version = "[^"]*"', 'version = "$(RELEASE_VERSION)"' | Set-Content $(TAURI_DIR)/Cargo.toml; (Get-Content package.json) -replace '"version": "[^"]*"', '"version": "$(RELEASE_VERSION)"' | Set-Content package.json; Set-Content -Path version -Value "$(RELEASE_VERSION)"; echo "Local files updated to $(RELEASE_VERSION)."; echo "https://github.com/b7s/plume/releases/tag/v$(RELEASE_VERSION)"

clean:
	@echo "Cleaning frontend artifacts..."
	Remove-Item -Recurse -Force -Path dist/ -ErrorAction SilentlyContinue
	Remove-Item -Recurse -Force -Path node_modules/.vite/ -ErrorAction SilentlyContinue
	@echo "Cleaning Rust artifacts..."
	Remove-Item -Recurse -Force -Path $(TAURI_DIR)/target/ -ErrorAction SilentlyContinue
	@echo "Clean complete."

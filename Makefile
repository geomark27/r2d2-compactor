# ================================================================
#  r2d2-compactor - Makefile
# ================================================================

BINARY      = r2d2-compactor
CARGO       = cargo
WIN_TARGET  = x86_64-pc-windows-gnu
BRANCH     := $(shell git branch --show-current 2>/dev/null || echo "main")
VERSION    := $(shell grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

CYAN=\033[0;36m
GREEN=\033[0;32m
YELLOW=\033[0;33m
RED=\033[0;31m
NC=\033[0m

.DEFAULT_GOAL=help

# ----------------------------------------------------------------
#  Help
# ----------------------------------------------------------------
.PHONY: help
help:
	@echo ""
	@echo "$(CYAN)  r2d2-compactor - Compresor de evidencias (v$(VERSION))$(NC)"
	@echo ""
	@echo "$(YELLOW)  🔨 Build & Run:$(NC)"
	@echo "    build           Compila release nativo (Linux, para probar en dev)"
	@echo "    build-windows   Cross-compila el .exe de Windows ($(WIN_TARGET))"
	@echo "    run             Ejecuta la app en modo desarrollo"
	@echo "    clean           Elimina artefactos de compilación"
	@echo ""
	@echo "$(YELLOW)  🧪 Calidad:$(NC)"
	@echo "    fmt             Formatea el código (cargo fmt)"
	@echo "    lint            cargo fmt --check + clippy sin warnings"
	@echo "    check           Verificación rápida (cargo check)"
	@echo ""
	@echo "$(YELLOW)  🚀 Release:$(NC)"
	@echo "    version         Muestra la versión actual"
	@echo "    release         Bump patch  + build + checksums + tag + GitHub Release"
	@echo "    release-minor   Bump minor  + ..."
	@echo "    release-major   Bump major  + ..."
	@echo ""
	@echo "$(YELLOW)  ⚙️  Setup único para cross-compilar a Windows:$(NC)"
	@echo "    rustup target add $(WIN_TARGET)"
	@echo "    sudo apt install mingw-w64"
	@echo ""

# ----------------------------------------------------------------
#  Build
# ----------------------------------------------------------------
.PHONY: build
build:
	@echo "$(YELLOW)Compilando release nativo...$(NC)"
	@$(CARGO) build --release
	@echo "$(GREEN)✓ target/release/$(BINARY)$(NC)"

.PHONY: build-windows
build-windows:
	@echo "$(YELLOW)Cross-compilando para Windows ($(WIN_TARGET))...$(NC)"
	@$(CARGO) build --release --target $(WIN_TARGET)
	@echo "$(GREEN)✓ target/$(WIN_TARGET)/release/$(BINARY).exe$(NC)"

.PHONY: run
run:
	@$(CARGO) run $(ARGS)

.PHONY: clean
clean:
	@$(CARGO) clean
	@echo "$(GREEN)✓ Limpio$(NC)"

# ----------------------------------------------------------------
#  Code quality
# ----------------------------------------------------------------
.PHONY: fmt
fmt:
	@$(CARGO) fmt
	@echo "$(GREEN)✓ Formateado$(NC)"

.PHONY: lint
lint:
	@$(CARGO) fmt --check
	@$(CARGO) clippy -- -D warnings
	@echo "$(GREEN)✓ Sin warnings$(NC)"

.PHONY: check
check:
	@$(CARGO) check

# ----------------------------------------------------------------
#  Versioning & Release
# ----------------------------------------------------------------
.PHONY: version
version:
	@echo "$(CYAN)Versión actual: v$(VERSION)$(NC)"

# Uso interno: `make _release BUMP=patch|minor|major`
.PHONY: _release
_release: lint
	@CURRENT="$(VERSION)"; \
	MAJOR=$$(echo $$CURRENT | cut -d. -f1); \
	MINOR=$$(echo $$CURRENT | cut -d. -f2); \
	PATCH=$$(echo $$CURRENT | cut -d. -f3); \
	case "$(BUMP)" in \
		major) MAJOR=$$((MAJOR+1)); MINOR=0; PATCH=0;; \
		minor) MINOR=$$((MINOR+1)); PATCH=0;; \
		*)     PATCH=$$((PATCH+1));; \
	esac; \
	NEW="$$MAJOR.$$MINOR.$$PATCH"; TAG="v$$NEW"; \
	echo "$(YELLOW)Release: v$$CURRENT → $$TAG$(NC)"; \
	sed -i "0,/^version = \".*\"/s//version = \"$$NEW\"/" Cargo.toml; \
	echo "$(YELLOW)[1/5]$(NC) Compilando binarios..."; \
	$(CARGO) build --release; \
	$(CARGO) build --release --target $(WIN_TARGET); \
	mkdir -p dist; \
	cp target/release/$(BINARY)                 dist/$(BINARY)-linux-amd64; \
	cp target/$(WIN_TARGET)/release/$(BINARY).exe dist/$(BINARY)-windows-amd64.exe; \
	echo "$(YELLOW)[2/5]$(NC) Generando checksums.txt..."; \
	( cd dist && sha256sum $(BINARY)-linux-amd64 $(BINARY)-windows-amd64.exe > checksums.txt ); \
	echo "$(YELLOW)[3/5]$(NC) Commit + tag $$TAG..."; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "release: $$TAG"; \
	git tag -a $$TAG -m "Release $$TAG"; \
	echo "$(YELLOW)[4/5]$(NC) Push a origin/$(BRANCH)..."; \
	git push origin $(BRANCH) --tags; \
	echo "$(YELLOW)[5/5]$(NC) Publicando en GitHub Releases..."; \
	gh release create $$TAG \
		dist/$(BINARY)-linux-amd64 \
		dist/$(BINARY)-windows-amd64.exe \
		dist/checksums.txt \
		--title "$$TAG" --notes "Release $$TAG"; \
	echo "$(GREEN)✓ Release $$TAG publicado$(NC)"

.PHONY: release
release:
	@$(MAKE) _release BUMP=patch

.PHONY: release-minor
release-minor:
	@$(MAKE) _release BUMP=minor

.PHONY: release-major
release-major:
	@$(MAKE) _release BUMP=major

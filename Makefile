# ================================================================
#  r2d2-compactor - Makefile
# ================================================================

BINARY      = r2d2-compactor
CARGO       = cargo
WIN_TARGET  = x86_64-pc-windows-gnu
VENDOR      = vendor/ffmpeg-win
FFMPEG_URL  = https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip
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
	@echo "    vendor-ffmpeg   Descarga FFmpeg de Windows a vendor/ (una vez)"
	@echo "    dist-windows    Arma el .zip de Windows (app + FFmpeg incluido)"
	@echo "    dist-installer  Arma el instalador setup.exe (asistente NSIS)"
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

# Descarga los binarios de FFmpeg para Windows y los deja en vendor/ (una vez).
.PHONY: vendor-ffmpeg
vendor-ffmpeg:
	@echo "$(YELLOW)Descargando FFmpeg para Windows (~110 MB)...$(NC)"
	@mkdir -p $(VENDOR)
	@curl -sL -o /tmp/ffmpeg-win.zip "$(FFMPEG_URL)"
	@unzip -o -j /tmp/ffmpeg-win.zip "*/bin/ffmpeg.exe" "*/bin/ffprobe.exe" -d $(VENDOR) >/dev/null
	@unzip -o -j /tmp/ffmpeg-win.zip "*/LICENSE*" -d $(VENDOR) >/dev/null || true
	@rm -f /tmp/ffmpeg-win.zip
	@echo "$(GREEN)✓ FFmpeg en $(VENDOR)$(NC)"

# Prepara en dist/pkg el contenido de la distribución de Windows (app + ffmpeg).
.PHONY: dist-pkg
dist-pkg: build-windows
	@if [ ! -f $(VENDOR)/ffmpeg.exe ]; then \
		echo "$(RED)Falta FFmpeg. Corre primero: make vendor-ffmpeg$(NC)"; exit 1; fi
	@rm -rf dist/pkg && mkdir -p dist/pkg/ffmpeg
	@cp target/$(WIN_TARGET)/release/$(BINARY).exe dist/pkg/
	@cp $(VENDOR)/ffmpeg.exe $(VENDOR)/ffprobe.exe dist/pkg/ffmpeg/
	@cp $(VENDOR)/LICENSE dist/pkg/ffmpeg/FFMPEG-LICENSE.txt 2>/dev/null || true

# Arma el .zip de distribución de Windows (versión "portable").
.PHONY: dist-windows
dist-windows: dist-pkg
	@( cd dist/pkg && zip -qr ../$(BINARY)-windows-amd64.zip . )
	@echo "$(GREEN)✓ dist/$(BINARY)-windows-amd64.zip (app + FFmpeg incluido)$(NC)"

# Arma el instalador de Windows (asistente NSIS: elegir carpeta, siguiente…).
.PHONY: dist-installer
dist-installer: dist-pkg
	@command -v makensis >/dev/null || { \
		echo "$(RED)Falta NSIS. Instala con: sudo apt install nsis$(NC)"; exit 1; }
	@ICO=$$(ls -t target/$(WIN_TARGET)/release/build/$(BINARY)-*/out/icon.ico 2>/dev/null | head -1); \
	if [ -n "$$ICO" ]; then ICODEF="-DAPP_ICO=$$(readlink -f $$ICO)"; else ICODEF=""; fi; \
	makensis -V2 -DVERSION=$(VERSION) -DPKG_DIR=$$(readlink -f dist/pkg) \
		-DOUTFILE=$$(readlink -f dist)/$(BINARY)-setup.exe $$ICODEF installer/installer.nsi
	@echo "$(GREEN)✓ dist/$(BINARY)-setup.exe (instalador con FFmpeg incluido)$(NC)"

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
	@if [ ! -f $(VENDOR)/ffmpeg.exe ]; then \
		echo "$(RED)Falta FFmpeg. Corre primero: make vendor-ffmpeg$(NC)"; exit 1; fi
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
	echo "$(YELLOW)[1/5]$(NC) Compilando binarios + zip + instalador de Windows..."; \
	$(CARGO) build --release; \
	$(MAKE) dist-windows; \
	$(MAKE) dist-installer; \
	cp target/release/$(BINARY)                 dist/$(BINARY)-linux-amd64; \
	cp target/$(WIN_TARGET)/release/$(BINARY).exe dist/$(BINARY)-windows-amd64.exe; \
	echo "$(YELLOW)[2/5]$(NC) Generando checksums.txt..."; \
	( cd dist && sha256sum $(BINARY)-linux-amd64 $(BINARY)-windows-amd64.exe $(BINARY)-windows-amd64.zip $(BINARY)-setup.exe > checksums.txt ); \
	echo "$(YELLOW)[3/5]$(NC) Commit + tag $$TAG..."; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "release: $$TAG"; \
	git tag -a $$TAG -m "Release $$TAG"; \
	echo "$(YELLOW)[4/5]$(NC) Push a origin/$(BRANCH)..."; \
	git push origin $(BRANCH) --tags; \
	echo "$(YELLOW)[5/5]$(NC) Publicando en GitHub Releases..."; \
	gh release create $$TAG \
		dist/$(BINARY)-setup.exe \
		dist/$(BINARY)-windows-amd64.zip \
		dist/$(BINARY)-windows-amd64.exe \
		dist/$(BINARY)-linux-amd64 \
		dist/checksums.txt \
		--title "$$TAG" \
		--notes "Release $$TAG. Para Windows descarga el instalador ($(BINARY)-setup.exe); el .zip es la versión portable. El .exe suelto es solo para la auto-actualización."; \
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

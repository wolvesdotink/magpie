#!/usr/bin/env bash
# build-macos.sh — Build and package Magpie as a macOS .app and .dmg
#
# Usage:
#   bash scripts/build-macos.sh [OPTIONS]
#
# Options:
#   --debug       Build in debug mode (default: release)
#   --universal   Build universal binary (aarch64 + x86_64)
#   --no-dmg      Skip DMG creation
#   --notarize    Submit the .app and .dmg to Apple notarization and staple
#                 the ticket. Requires APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
#                 in the environment. Implies that APPLE_SIGNING_IDENTITY is
#                 a real Developer ID identity (not the ad-hoc "-").
#   --updater     After the app is signed (and notarized if --notarize was
#                 also passed), produce the Tauri updater payload:
#                   - <APP_NAME>.app.tar.gz
#                   - <APP_NAME>.app.tar.gz.sig
#                 Requires TAURI_SIGNING_PRIVATE_KEY in the environment.
#                 TAURI_SIGNING_PRIVATE_KEY_PASSWORD optional (blank if the
#                 key was generated without a password).
#   --clean       Clean build artifacts before building
#   --verbose     Enable verbose output
#
# Environment:
#   APPLE_SIGNING_IDENTITY              Code signing identity (default: ad-hoc "-")
#   APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID   Required for --notarize
#   TAURI_SIGNING_PRIVATE_KEY           Required for --updater
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD  Optional, if the key has a password

set -euo pipefail

# ─── Configuration ───────────────────────────────────────────────────────────

APP_NAME="Magpie"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$PROJECT_ROOT/src-tauri"

# Defaults
PROFILE="release"
BUILD_FLAGS=()
UNIVERSAL=false
SKIP_DMG=false
NOTARIZE=false
UPDATER=false
VERBOSE=false
CLEAN=false

# ─── Argument Parsing ────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            PROFILE="debug"
            BUILD_FLAGS+=("--debug")
            shift
            ;;
        --universal)
            UNIVERSAL=true
            shift
            ;;
        --no-dmg)
            SKIP_DMG=true
            shift
            ;;
        --notarize)
            NOTARIZE=true
            shift
            ;;
        --updater)
            UPDATER=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            BUILD_FLAGS+=("--verbose")
            shift
            ;;
        --help|-h)
            head -16 "$0" | tail -14
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# ─── Derived Paths ───────────────────────────────────────────────────────────

if $UNIVERSAL; then
    TARGET_TRIPLE="universal-apple-darwin"
    TARGET_DIR="$TAURI_DIR/target/$TARGET_TRIPLE/$PROFILE"
else
    TARGET_DIR="$TAURI_DIR/target/$PROFILE"
fi

APP_BUNDLE="$TARGET_DIR/bundle/macos/$APP_NAME.app"
FRAMEWORKS_DIR="$APP_BUNDLE/Contents/Frameworks"
BINARY="$APP_BUNDLE/Contents/MacOS/$APP_NAME"
SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:--}"

# ─── Helpers ─────────────────────────────────────────────────────────────────

log()  { echo "==> $*"; }
info() { echo "    $*"; }
warn() { echo "⚠️  $*"; }
err()  { echo "❌  $*" >&2; exit 1; }
verbose() { $VERBOSE && echo "    [verbose] $*" || true; }

# ─── Phase 1: Prerequisites ─────────────────────────────────────────────────

log "Checking prerequisites..."

command -v pnpm  >/dev/null 2>&1 || err "pnpm is not installed. Install from https://pnpm.io"
command -v cargo >/dev/null 2>&1 || err "cargo is not installed. Install from https://rustup.rs"
command -v otool >/dev/null 2>&1 || err "otool not found. Install Xcode Command Line Tools."

if $UNIVERSAL; then
    rustup target list --installed | grep -q "aarch64-apple-darwin" \
        || err "Missing Rust target: aarch64-apple-darwin (run: rustup target add aarch64-apple-darwin)"
    rustup target list --installed | grep -q "x86_64-apple-darwin" \
        || err "Missing Rust target: x86_64-apple-darwin (run: rustup target add x86_64-apple-darwin)"
    log "Universal binary: both arm64 and x86_64 targets available"
fi

if $NOTARIZE; then
    [[ "$SIGNING_IDENTITY" != "-" ]] \
        || err "--notarize requires APPLE_SIGNING_IDENTITY (cannot notarize an ad-hoc signed app)"
    : "${APPLE_ID:?--notarize requires APPLE_ID env var}"
    : "${APPLE_PASSWORD:?--notarize requires APPLE_PASSWORD env var (app-specific password)}"
    : "${APPLE_TEAM_ID:?--notarize requires APPLE_TEAM_ID env var}"
    command -v xcrun >/dev/null 2>&1 \
        || err "xcrun not found. Install Xcode Command Line Tools for notarytool/stapler."
fi

if $UPDATER; then
    : "${TAURI_SIGNING_PRIVATE_KEY:?--updater requires TAURI_SIGNING_PRIVATE_KEY env var}"
fi

info "pnpm $(pnpm --version), cargo $(cargo --version | awk '{print $2}')"

# ─── Phase 2: Clean (optional) ──────────────────────────────────────────────

if $CLEAN; then
    log "Cleaning build artifacts..."
    (cd "$TAURI_DIR" && cargo clean)
fi

# ─── Phase 3: Build ─────────────────────────────────────────────────────────

log "Building $APP_NAME ($PROFILE)..."

BUILD_CMD=(pnpm tauri build --bundles app)

if $UNIVERSAL; then
    BUILD_CMD+=(--target universal-apple-darwin)
fi

# `createUpdaterArtifacts` in tauri.conf.json makes `tauri build` produce and
# sign the updater .tar.gz during the build itself, which hard-fails without
# TAURI_SIGNING_PRIVATE_KEY. Keyless local builds (the README's plain
# `pnpm run build:mac`) don't need that artifact — and the --updater flow
# signs it explicitly in Phase 7 anyway — so skip it when no key is present.
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
    log "TAURI_SIGNING_PRIVATE_KEY not set — skipping in-build updater artifact"
    BUILD_CMD+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi

if [[ ${#BUILD_FLAGS[@]} -gt 0 ]]; then
    BUILD_CMD+=("${BUILD_FLAGS[@]}")
fi

verbose "Running: ${BUILD_CMD[*]}"
(cd "$PROJECT_ROOT" && "${BUILD_CMD[@]}")

if [[ ! -d "$APP_BUNDLE" ]]; then
    err "Build failed: $APP_BUNDLE not found"
fi

log "App bundle created at $APP_BUNDLE"

# ─── Phase 4: Discover Dylibs ───────────────────────────────────────────────

log "Discovering dynamic libraries..."

DYLIB_SEARCH_DIR="$TARGET_DIR"
if $UNIVERSAL; then
    # For universal builds, dylibs may be in architecture-specific dirs
    # Check the main target dir first
    DYLIB_SEARCH_DIR="$TARGET_DIR"
fi

# Find llama/ggml dylibs (real files only, not symlinks)
DYLIB_FILES=()
DYLIB_SYMLINKS=()

while IFS= read -r -d '' f; do
    if [[ -L "$f" ]]; then
        DYLIB_SYMLINKS+=("$f")
    else
        DYLIB_FILES+=("$f")
    fi
done < <(find "$DYLIB_SEARCH_DIR" -maxdepth 1 \( -name "libllama*.dylib" -o -name "libggml*.dylib" \) -print0 2>/dev/null)

TOTAL_DYLIBS=$(( ${#DYLIB_FILES[@]} + ${#DYLIB_SYMLINKS[@]} ))

if [[ $TOTAL_DYLIBS -eq 0 ]]; then
    warn "No llama/ggml dylibs found in $DYLIB_SEARCH_DIR"
    warn "Build appears to use static linking. Skipping dylib bundling."
    warn "If this is unexpected, try: --clean to force a fresh build"
else
    log "Found $TOTAL_DYLIBS dylibs (${#DYLIB_FILES[@]} files, ${#DYLIB_SYMLINKS[@]} symlinks)"

    for f in "${DYLIB_FILES[@]}"; do
        verbose "  file: $(basename "$f") ($(du -h "$f" | awk '{print $1}'))"
    done
    for f in "${DYLIB_SYMLINKS[@]}"; do
        verbose "  link: $(basename "$f") -> $(readlink "$f")"
    done

    # ─── Phase 5: Bundle Dylibs into Frameworks ─────────────────────────────

    log "Bundling dylibs into $APP_NAME.app/Contents/Frameworks/..."
    mkdir -p "$FRAMEWORKS_DIR"

    # Copy real files first
    for dylib in "${DYLIB_FILES[@]}"; do
        name="$(basename "$dylib")"
        cp "$dylib" "$FRAMEWORKS_DIR/$name"
        verbose "Copied $name"
    done

    # Recreate symlinks
    for dylib in "${DYLIB_SYMLINKS[@]}"; do
        name="$(basename "$dylib")"
        link_target="$(readlink "$dylib")"
        ln -sf "$link_target" "$FRAMEWORKS_DIR/$name"
        verbose "Linked $name -> $link_target"
    done

    # ─── Phase 6: Verify & Fix Dylib References ─────────────────────────────

    log "Verifying dylib references..."

    FIXUPS_APPLIED=0

    # 6a. Ensure the binary has the Frameworks rpath
    if ! otool -l "$BINARY" | grep -A2 "LC_RPATH" | grep -q "@executable_path/../Frameworks"; then
        info "Adding @executable_path/../Frameworks rpath to binary"
        install_name_tool -add_rpath "@executable_path/../Frameworks" "$BINARY"
        FIXUPS_APPLIED=$((FIXUPS_APPLIED + 1))
    else
        verbose "Binary already has @executable_path/../Frameworks rpath"
    fi

    # 6b. Fix references in the main binary
    while IFS= read -r line; do
        lib_ref="$(echo "$line" | awk '{print $1}')"
        # Skip @rpath and @executable_path references — they're already correct
        if [[ "$lib_ref" == @rpath/* ]] || [[ "$lib_ref" == @executable_path/* ]]; then
            verbose "OK: $lib_ref"
            continue
        fi
        lib_name="$(basename "$lib_ref")"
        info "Fixing binary reference: $lib_ref -> @rpath/$lib_name"
        install_name_tool -change "$lib_ref" "@rpath/$lib_name" "$BINARY"
        FIXUPS_APPLIED=$((FIXUPS_APPLIED + 1))
    done < <(otool -L "$BINARY" | grep -E "libllama|libggml" | sed 's/^[[:space:]]*//')

    # 6c. Fix dylib install names and inter-library references
    for dylib in "$FRAMEWORKS_DIR"/lib{llama,ggml}*.dylib; do
        [[ -e "$dylib" ]] || continue
        [[ -L "$dylib" ]] && continue  # skip symlinks

        name="$(basename "$dylib")"
        verbose "Checking $name..."

        # Fix the dylib's own install name (id)
        current_id="$(otool -D "$dylib" | tail -1)"
        if [[ "$current_id" != @rpath/* ]]; then
            # Use the .0.dylib short name convention if possible
            short_name="$name"
            # Try to extract a shorter versioned name (e.g., libllama.0.dylib from libllama.0.0.0.dylib)
            if [[ "$name" =~ ^(lib[a-z-]+)\.[0-9]+\.[0-9]+\.[0-9]+\.dylib$ ]]; then
                major_ver="$(echo "$name" | sed -E 's/^lib[a-z-]+\.([0-9]+)\..*/\1/')"
                base="$(echo "$name" | sed -E 's/^(lib[a-z-]+)\..*/\1/')"
                short_name="${base}.${major_ver}.dylib"
            elif [[ "$name" =~ ^(lib[a-z-]+)\.[0-9]+\.[0-9]+\.dylib$ ]]; then
                major_ver="$(echo "$name" | sed -E 's/^lib[a-z-]+\.([0-9]+)\..*/\1/')"
                base="$(echo "$name" | sed -E 's/^(lib[a-z-]+)\..*/\1/')"
                short_name="${base}.${major_ver}.dylib"
            fi
            info "Setting install name: $name -> @rpath/$short_name"
            install_name_tool -id "@rpath/$short_name" "$dylib"
            FIXUPS_APPLIED=$((FIXUPS_APPLIED + 1))
        else
            verbose "Install name OK: $current_id"
        fi

        # Fix inter-library references
        while IFS= read -r line; do
            lib_ref="$(echo "$line" | awk '{print $1}')"
            if [[ "$lib_ref" == @rpath/* ]] || [[ "$lib_ref" == @executable_path/* ]]; then
                continue
            fi
            ref_name="$(basename "$lib_ref")"
            info "Fixing $name reference: $lib_ref -> @rpath/$ref_name"
            install_name_tool -change "$lib_ref" "@rpath/$ref_name" "$dylib"
            FIXUPS_APPLIED=$((FIXUPS_APPLIED + 1))
        done < <(otool -L "$dylib" | grep -E "libllama|libggml" | sed 's/^[[:space:]]*//')
    done

    if [[ $FIXUPS_APPLIED -gt 0 ]]; then
        log "Applied $FIXUPS_APPLIED install_name_tool fixups"
    else
        log "All dylib references are correct — no fixups needed"
    fi

    # ─── Phase 7: Codesign ───────────────────────────────────────────────────

    ENTITLEMENTS="$TAURI_DIR/entitlements.plist"
    if [[ ! -f "$ENTITLEMENTS" ]]; then
        err "Entitlements file not found: $ENTITLEMENTS"
    fi

    log "Code signing ($( [[ "$SIGNING_IDENTITY" == "-" ]] && echo "ad-hoc" || echo "$SIGNING_IDENTITY" ))..."
    info "Entitlements: $ENTITLEMENTS"

    # Sign dylibs first (inside-out order; dylibs don't carry entitlements)
    for dylib in "$FRAMEWORKS_DIR"/lib{llama,ggml}*.dylib; do
        [[ -e "$dylib" ]] || continue
        [[ -L "$dylib" ]] && continue  # skip symlinks
        codesign --force --sign "$SIGNING_IDENTITY" --options runtime "$dylib"
        verbose "Signed $(basename "$dylib")"
    done

    # Sign the main executable with entitlements and hardened runtime
    codesign --force --sign "$SIGNING_IDENTITY" --options runtime \
        --entitlements "$ENTITLEMENTS" "$BINARY"
    verbose "Signed main executable with entitlements"

    # Sign the app bundle (seals resources)
    codesign --force --sign "$SIGNING_IDENTITY" --options runtime \
        --entitlements "$ENTITLEMENTS" "$APP_BUNDLE"
    log "Code signing complete"
fi

# ─── Phase 7.5: Notarize the .app ───────────────────────────────────────────
#
# Notarization must happen on a fully-signed bundle, including any dylibs
# we just bundled into Frameworks/. Apple's notarytool zips the .app for
# upload, scans for Hardened Runtime + valid signatures, and waits for the
# verdict. Stapling embeds the resulting ticket so Gatekeeper can verify
# offline on first launch.

if $NOTARIZE; then
    log "Notarizing $APP_NAME.app (this can take a few minutes)..."

    NOTARIZE_ZIP="$(mktemp -t magpie-notarize-XXXXXX).zip"
    trap 'rm -f "$NOTARIZE_ZIP"' EXIT

    # ditto preserves resource forks + extended attributes; zip(1) does not.
    /usr/bin/ditto -c -k --keepParent "$APP_BUNDLE" "$NOTARIZE_ZIP"

    xcrun notarytool submit "$NOTARIZE_ZIP" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id  "$APPLE_TEAM_ID" \
        --wait

    rm -f "$NOTARIZE_ZIP"

    log "Stapling notarization ticket to $APP_NAME.app..."
    xcrun stapler staple "$APP_BUNDLE"
    xcrun stapler validate "$APP_BUNDLE"
fi

# ─── Phase 7.7: Generate updater payload ────────────────────────────────────
#
# Tauri's updater plugin downloads a gzipped tarball of the new .app and
# verifies it against a minisign signature embedded in latest.json. We do
# this manually (instead of letting `tauri build --bundles updater` do it
# during cargo's bundle step) because the bundle step happens BEFORE we've
# injected our llama/ggml dylibs — so its tarball would be missing those.
# Generating the payload here ensures the .tar.gz reflects the
# fully-bundled, signed, and (optionally) notarized .app.

if $UPDATER; then
    log "Generating updater payload..."

    BUNDLE_MACOS_DIR="$(dirname "$APP_BUNDLE")"
    UPDATER_TARBALL="$BUNDLE_MACOS_DIR/$APP_NAME.app.tar.gz"

    # Tar from the bundle's parent so the archive has $APP_NAME.app at root.
    (
        cd "$BUNDLE_MACOS_DIR"
        # COPYFILE_DISABLE strips ._* AppleDouble metadata files that Finder
        # adds to network volumes — they break tar reproducibility.
        COPYFILE_DISABLE=1 tar -czf "$APP_NAME.app.tar.gz" "$APP_NAME.app"
    )

    log "Signing updater payload with Tauri minisign key..."

    # `tauri signer sign` looks up the private key via TAURI_SIGNING_PRIVATE_KEY
    # (the same env var the bundle step uses). It writes <FILE>.sig next to
    # the input file.
    TAURI_SIGNING_PRIVATE_KEY="$TAURI_SIGNING_PRIVATE_KEY" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" \
    pnpm tauri signer sign \
        --private-key "$TAURI_SIGNING_PRIVATE_KEY" \
        --password "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" \
        "$UPDATER_TARBALL"

    [[ -f "${UPDATER_TARBALL}.sig" ]] || err "Updater signature not produced."

    log "Updater payload: $UPDATER_TARBALL"
    log "Updater signature: ${UPDATER_TARBALL}.sig"
fi

# ─── Phase 8: Create DMG ────────────────────────────────────────────────────

if $SKIP_DMG; then
    log "Skipping DMG creation (--no-dmg)"
    DMG_PATH=""
else
    # Styled DMG: branded background, custom volume icon, drop-link to /Applications.
    # Window size, icon-size, and icon positions mirror src-tauri/tauri.conf.json's
    # bundle.macOS.dmg block — keep them in sync if you change either one.
    if ! command -v create-dmg >/dev/null; then
        err "create-dmg not found. Install with: brew install create-dmg"
    fi

    log "Creating styled DMG..."

    VERSION="$(python3 -c "import json; print(json.load(open('$PROJECT_ROOT/package.json'))['version'])")"
    DMG_NAME="${APP_NAME}_${VERSION}_$(uname -m).dmg"
    DMG_PATH="$TARGET_DIR/bundle/dmg/$DMG_NAME"

    BACKGROUND="$PROJECT_ROOT/installer/dmg-background.tiff"
    VOLUME_ICON="$PROJECT_ROOT/installer/volume-icon.icns"
    [[ -f "$VOLUME_ICON" ]] || VOLUME_ICON="$PROJECT_ROOT/src-tauri/icons/icon.icns"

    [[ -f "$BACKGROUND" ]]   || err "Missing DMG background asset: $BACKGROUND. Run installer/build-assets.sh."
    [[ -f "$VOLUME_ICON" ]]  || err "Missing volume icon: $VOLUME_ICON"

    # create-dmg copies the *contents* of its source folder into the disk image,
    # so we stage just the .app inside a temp dir. The Applications drop link
    # is added by create-dmg via --app-drop-link (no manual symlink needed).
    DMG_STAGING="$(mktemp -d)"
    trap "rm -rf '$DMG_STAGING'" EXIT
    cp -R "$APP_BUNDLE" "$DMG_STAGING/"

    mkdir -p "$(dirname "$DMG_PATH")"
    rm -f "$DMG_PATH"

    create-dmg \
        --volname "$APP_NAME" \
        --volicon "$VOLUME_ICON" \
        --background "$BACKGROUND" \
        --window-pos 200 120 \
        --window-size 660 400 \
        --icon-size 128 \
        --text-size 13 \
        --icon "$APP_NAME.app" 180 170 \
        --hide-extension "$APP_NAME.app" \
        --app-drop-link 480 170 \
        --no-internet-enable \
        "$DMG_PATH" \
        "$DMG_STAGING"

    log "DMG created at $DMG_PATH"

    # ─── Phase 8.5: Sign + notarize the DMG ─────────────────────────────────
    #
    # Even with a fully-notarized .app inside, the DMG itself needs its own
    # signature + notarization for Gatekeeper to accept it without warnings
    # when the user double-clicks the disk image. Skipping this is the most
    # common reason a notarized app still triggers "from an unidentified
    # developer" on first download.

    if [[ "$SIGNING_IDENTITY" != "-" ]]; then
        log "Code signing DMG..."
        codesign --force --sign "$SIGNING_IDENTITY" --timestamp "$DMG_PATH"
        codesign --verify --verbose=2 "$DMG_PATH"
    fi

    if $NOTARIZE; then
        log "Notarizing DMG..."
        xcrun notarytool submit "$DMG_PATH" \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id  "$APPLE_TEAM_ID" \
            --wait

        log "Stapling notarization ticket to DMG..."
        xcrun stapler staple "$DMG_PATH"
        xcrun stapler validate "$DMG_PATH"
    fi
fi

# ─── Phase 9: Summary ───────────────────────────────────────────────────────

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Build complete!                                            ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "  App:  $APP_BUNDLE"
echo "        $(du -sh "$APP_BUNDLE" | awk '{print $1}')"

if [[ -n "${DMG_PATH:-}" ]] && [[ -f "$DMG_PATH" ]]; then
    echo "  DMG:  $DMG_PATH"
    echo "        $(du -sh "$DMG_PATH" | awk '{print $1}')"
fi

if [[ -d "$FRAMEWORKS_DIR" ]]; then
    FRAMEWORK_COUNT="$(find "$FRAMEWORKS_DIR" -name "*.dylib" ! -type l 2>/dev/null | wc -l | tr -d ' ')"
    echo ""
    echo "  Bundled dylibs: $FRAMEWORK_COUNT"
fi

# Confirm Apple Silicon acceleration frameworks are linked in.
# whisper-rs's build script links CoreML.framework + Metal.framework when
# the matching Cargo features are enabled. If they go missing, the app
# silently falls back to CPU — so surface it loudly here.
if [[ -f "$BINARY" ]]; then
    LINKED_FRAMEWORKS="$(otool -L "$BINARY" 2>/dev/null || true)"
    echo ""
    if echo "$LINKED_FRAMEWORKS" | grep -q "/CoreML.framework/"; then
        echo "  CoreML linkage: yes"
    else
        warn "CoreML.framework is NOT linked. Encoder will not run on the ANE."
    fi
    if echo "$LINKED_FRAMEWORKS" | grep -q "/Metal.framework/"; then
        echo "  Metal linkage:  yes"
    else
        warn "Metal.framework is NOT linked. Whisper will run on CPU only."
    fi
fi

if $UPDATER; then
    BUNDLE_MACOS_DIR="$(dirname "$APP_BUNDLE")"
    UPDATER_TARBALL="$BUNDLE_MACOS_DIR/$APP_NAME.app.tar.gz"
    if [[ -f "$UPDATER_TARBALL" ]]; then
        echo ""
        echo "  Updater payload:    $UPDATER_TARBALL"
        echo "                      $(du -sh "$UPDATER_TARBALL" | awk '{print $1}')"
    fi
    if [[ -f "${UPDATER_TARBALL}.sig" ]]; then
        echo "  Updater signature:  ${UPDATER_TARBALL}.sig"
    fi
fi

if $NOTARIZE; then
    echo ""
    echo "  Notarized: yes (stapled to .app$( [[ -f "${DMG_PATH:-}" ]] && echo " and .dmg" ))"
fi

echo ""

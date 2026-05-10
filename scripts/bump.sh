#!/usr/bin/env bash
# bump.sh — bump magpie's version across package.json, Cargo.toml, Cargo.lock,
# and tauri.conf.json, then commit, tag, and push. Pushing the tag triggers
# the release workflow in .github/workflows/release.yml.
#
# Usage:
#   bash scripts/bump.sh patch
#   bash scripts/bump.sh minor
#   bash scripts/bump.sh major

set -euo pipefail

BUMP_TYPE="${1:-}"

usage() {
  echo "Usage: $0 [major|minor|patch]"
  exit 1
}

if [[ -z "$BUMP_TYPE" ]]; then usage; fi
case "$BUMP_TYPE" in major|minor|patch) ;; *) usage ;; esac

# Ensure clean working tree.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree has uncommitted changes. Commit or stash them first."
  exit 1
fi

# Read current version from package.json.
CURRENT=$(node -p "require('./package.json').version")

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP_TYPE" in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
esac

NEW="$MAJOR.$MINOR.$PATCH"
TAG="v$NEW"

echo "Bumping $CURRENT → $NEW"

# package.json
node -e "
  const fs = require('fs');
  const p = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  p.version = '$NEW';
  fs.writeFileSync('package.json', JSON.stringify(p, null, 2) + '\n');
"

# src-tauri/Cargo.toml — only the first occurrence (the [package] version).
awk -v cur="$CURRENT" -v new="$NEW" '
  !done && $0 == "version = \"" cur "\"" { print "version = \"" new "\""; done=1; next }
  { print }
' src-tauri/Cargo.toml > src-tauri/Cargo.toml.tmp && mv src-tauri/Cargo.toml.tmp src-tauri/Cargo.toml

# src-tauri/tauri.conf.json
node -e "
  const fs = require('fs');
  const c = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  c.version = '$NEW';
  fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(c, null, 2) + '\n');
"

# Refresh Cargo.lock so the magpie package version matches Cargo.toml.
(cd src-tauri && cargo update -p magpie --precise "$NEW")

# Commit and tag.
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "chore: bump version to $NEW"
git tag "$TAG"
git push origin HEAD
git push origin "$TAG"

echo "Released $TAG"

#!/usr/bin/env bash
# bump.sh — bump magpie's version across package.json, Cargo.toml, Cargo.lock,
# and tauri.conf.json, then commit, tag, and push. Pushing the tag triggers
# the release workflow in .github/workflows/release.yml.
#
# Stable bumps:
#   bash scripts/bump.sh patch        # 0.1.17 → 0.1.18
#   bash scripts/bump.sh minor        # 0.1.17 → 0.2.0
#   bash scripts/bump.sh major        # 0.1.17 → 1.0.0
#
# Beta bumps (cuts a prerelease at the chosen level):
#   bash scripts/bump.sh patch --beta # 0.1.17 → 0.1.18-beta.1
#   bash scripts/bump.sh minor --beta # 0.1.17 → 0.2.0-beta.1
#   bash scripts/bump.sh major --beta # 0.1.17 → 1.0.0-beta.1
#
# Next beta in current cycle (must already be on a -beta.N version):
#   bash scripts/bump.sh beta         # 0.1.18-beta.1 → 0.1.18-beta.2
#
# Promote current beta to stable (strips -beta.N, keeps the X.Y.Z):
#   bash scripts/bump.sh stable       # 0.1.18-beta.7 → 0.1.18
#
# Stripping the -beta.N suffix also happens automatically when you bump a
# level: `bash scripts/bump.sh patch` from `0.1.18-beta.7` produces `0.1.19`,
# not `0.1.18`. Use `stable` when you want to release the current beta as-is.

set -euo pipefail

usage() {
  echo "Usage: $0 (major|minor|patch) [--beta]"
  echo "       $0 beta"
  echo "       $0 stable"
  exit 1
}

BUMP_TYPE=""
BETA_FLAG=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    major|minor|patch|beta|stable)
      if [[ -n "$BUMP_TYPE" ]]; then usage; fi
      BUMP_TYPE="$1"
      shift
      ;;
    --beta)
      BETA_FLAG=true
      shift
      ;;
    *)
      usage
      ;;
  esac
done

if [[ -z "$BUMP_TYPE" ]]; then usage; fi

# The `beta` subcommand bumps an existing -beta.N counter; `stable` strips
# it; `--beta` is a flag for major/minor/patch that appends -beta.1 after
# the level bump. None of them compose — combining would just be confusing.
if [[ "$BUMP_TYPE" == "beta" && "$BETA_FLAG" == "true" ]]; then
  echo "Error: the 'beta' subcommand does not accept --beta."
  echo "Use 'beta' alone to advance the counter, or '<level> --beta' to start a new beta cycle."
  exit 1
fi
if [[ "$BUMP_TYPE" == "stable" && "$BETA_FLAG" == "true" ]]; then
  echo "Error: the 'stable' subcommand does not accept --beta."
  echo "Use 'stable' alone to promote the current beta, or '<level> --beta' to start a new beta cycle."
  exit 1
fi

# Ensure clean working tree.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree has uncommitted changes. Commit or stash them first."
  exit 1
fi

# Read current version from package.json.
CURRENT=$(node -p "require('./package.json').version")

# Parse current into base (X.Y.Z) + optional -beta.N. We support exactly the
# shape `X.Y.Z` and `X.Y.Z-beta.N`; anything else means someone hand-edited
# the file into a state we don't know how to advance, and we'd rather error
# than silently produce a nonsense tag.
if [[ "$CURRENT" =~ ^([0-9]+\.[0-9]+\.[0-9]+)-beta\.([0-9]+)$ ]]; then
  BASE="${BASH_REMATCH[1]}"
  BETA_N="${BASH_REMATCH[2]}"
elif [[ "$CURRENT" =~ ^([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  BASE="${BASH_REMATCH[1]}"
  BETA_N=""
else
  echo "Error: cannot parse current version '$CURRENT' (expected X.Y.Z or X.Y.Z-beta.N)."
  exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$BASE"

case "$BUMP_TYPE" in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0; NEW="$MAJOR.$MINOR.$PATCH" ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0;          NEW="$MAJOR.$MINOR.$PATCH" ;;
  patch) PATCH=$((PATCH + 1));                   NEW="$MAJOR.$MINOR.$PATCH" ;;
  beta)
    if [[ -z "$BETA_N" ]]; then
      echo "Error: current version $CURRENT is not a beta."
      echo "Use bump:patch:beta, bump:minor:beta, or bump:major:beta to start a beta cycle."
      exit 1
    fi
    NEW_BETA=$((BETA_N + 1))
    NEW="$BASE-beta.$NEW_BETA"
    ;;
  stable)
    if [[ -z "$BETA_N" ]]; then
      echo "Error: current version $CURRENT is not a beta — nothing to promote."
      exit 1
    fi
    NEW="$BASE"
    ;;
esac

if [[ "$BUMP_TYPE" != "beta" ]] && $BETA_FLAG; then
  NEW="${NEW}-beta.1"
fi

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

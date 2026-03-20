#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 MAJOR|MINOR|PATCH"
  exit 1
}

[[ $# -eq 1 ]] || usage

BUMP="${1^^}" # uppercase
[[ "$BUMP" =~ ^(MAJOR|MINOR|PATCH)$ ]] || usage

MIXFILE="mix.exs"

# Ensure there are no uncommitted changes that would be left out of the release
if [[ -n "$(git diff --name-only)" || -n "$(git diff --cached --name-only)" ]]; then
  echo "ERROR: There are uncommitted changes. Please commit or stash them before releasing."
  exit 1
fi

# Extract current version
CURRENT=$(grep -oP '@version "\K[0-9]+\.[0-9]+\.[0-9]+' "$MIXFILE")
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP" in
  MAJOR) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  MINOR) MINOR=$((MINOR + 1)); PATCH=0 ;;
  PATCH) PATCH=$((PATCH + 1)) ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
TAG="v${NEW_VERSION}"

echo "==> Bumping version: $CURRENT -> $NEW_VERSION"

# 1. Update version in mix.exs
sed -i "s/@version \"$CURRENT\"/@version \"$NEW_VERSION\"/" "$MIXFILE"

# 2. Commit, tag, push
git add "$MIXFILE"
git commit -m "Bump version to $NEW_VERSION"
git tag "$TAG"

echo "==> Pushing commit and tag $TAG"
git push origin HEAD --tags

# 3. Wait for CI to build precompiled NIFs
echo "==> Waiting for CI workflow to complete for $TAG..."
REPO="Environmental-Material-Science-Inc/ex_gdal"

# Wait for the workflow run to appear
for i in $(seq 1 30); do
  RUN_ID=$(gh run list --repo "$REPO" --branch "$TAG" --workflow "Build precompiled NIFs" --json databaseId,status -q '.[0].databaseId' 2>/dev/null || true)
  [[ -n "$RUN_ID" ]] && break
  echo "  waiting for workflow run to start... (attempt $i)"
  sleep 10
done

if [[ -z "${RUN_ID:-}" ]]; then
  echo "ERROR: CI workflow did not start after 5 minutes. Check GitHub Actions."
  exit 1
fi

echo "==> Watching workflow run $RUN_ID"
gh run watch "$RUN_ID" --repo "$REPO" --exit-status

# 4. Generate checksums from release assets
echo "==> Downloading release assets and generating checksums"
CHECKSUM_FILE="checksum-Elixir.ExGdal.Native.exs"
TMPDIR=$(mktemp -d)
gh release download "$TAG" --repo "$REPO" --pattern "*.tar.gz" --dir "$TMPDIR"

{
  echo "%{"
  for f in "$TMPDIR"/*.tar.gz; do
    name=$(basename "$f")
    hash=$(sha256sum "$f" | cut -d' ' -f1)
    echo "  \"${name}\" => \"sha256:${hash}\","
  done
  echo "}"
} > "$CHECKSUM_FILE"

rm -rf "$TMPDIR"

echo "==> Checksums written to $CHECKSUM_FILE:"
cat "$CHECKSUM_FILE"

# 5. Publish to Hex
echo ""
echo "==> Review checksums above, then publish:"
echo ""
echo "    mix hex.publish"
echo ""

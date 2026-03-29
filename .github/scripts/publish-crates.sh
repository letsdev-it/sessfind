#!/usr/bin/env bash
# Compare Cargo.toml version with crates.io and publish sessfind.
# Expects: CARGO_REGISTRY_TOKEN, GITHUB_TOKEN (for optional version bump push).

set -euo pipefail

CRATE_NAME="sessfind"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "error: CARGO_REGISTRY_TOKEN is not set"
  exit 1
fi

parse_triple() {
  local v="$1"
  if [[ "$v" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    echo "${BASH_REMATCH[1]} ${BASH_REMATCH[2]} ${BASH_REMATCH[3]}"
  else
    echo "error: expected semver X.Y.Z, got: $v" >&2
    exit 1
  fi
}

compare_xy() {
  # echo -1 if a_xy < b_xy, 0 if equal, 1 if a_xy > b_xy
  local am an bm bn
  am="$1" an="$2" bm="$3" bn="$4"
  if (( am < bm )) || (( am == bm && an < bn )); then
    echo -1
  elif (( am > bm )) || (( am == bm && an > bn )); then
    echo 1
  else
    echo 0
  fi
}

compare_full() {
  # -1 if a < b, 0 equal, 1 if a > b
  local am an ap bm bn bp
  am="$1" an="$2" ap="$3" bm="$4" bn="$5" bp="$6"
  if (( am < bm )); then echo -1; return; fi
  if (( am > bm )); then echo 1; return; fi
  if (( an < bn )); then echo -1; return; fi
  if (( an > bn )); then echo 1; return; fi
  if (( ap < bp )); then echo -1; return; fi
  if (( ap > bp )); then echo 1; return; fi
  echo 0
}

MAIN_VER=$(cargo metadata --no-deps --format-version 1 | jq -r --arg n "$CRATE_NAME" '.packages[] | select(.name == $n) | .version')
echo "Cargo.toml ($CRATE_NAME): $MAIN_VER"

read -r MAIN_M MAIN_N MAIN_P <<<"$(parse_triple "$MAIN_VER")"

UA="sessfind-ci (https://github.com/letsdev-it/sessfind; publish-crates workflow)"
HTTP_CODE=$(curl -sS -A "$UA" -o /tmp/crates-api.json -w "%{http_code}" "https://crates.io/api/v1/crates/${CRATE_NAME}")

CRATES_VER=""
if [[ "$HTTP_CODE" == "200" ]]; then
  if jq -e '.errors' /tmp/crates-api.json >/dev/null 2>&1; then
    echo "error: unexpected API errors payload"
    jq . /tmp/crates-api.json >&2 || true
    exit 1
  fi
  CRATES_VER=$(jq -r '.crate.max_version // empty' /tmp/crates-api.json)
fi

if [[ "$HTTP_CODE" == "404" ]] || [[ -z "$CRATES_VER" ]]; then
  echo "No published version on crates.io (or crate missing); publishing $MAIN_VER as-is."
  cargo publish --token "$CARGO_REGISTRY_TOKEN"
  exit 0
fi

read -r CRATES_M CRATES_N CRATES_P <<<"$(parse_triple "$CRATES_VER")"
echo "crates.io latest: $CRATES_VER"

XY_CMP=$(compare_xy "$MAIN_M" "$MAIN_N" "$CRATES_M" "$CRATES_N")

if [[ "$XY_CMP" == -1 ]]; then
  echo "error: main major.minor ($MAIN_M.$MAIN_N) is less than crates.io ($CRATES_M.$CRATES_N) — refusing to publish (2c)."
  exit 1
fi

if [[ "$XY_CMP" == 1 ]]; then
  echo "main major.minor > crates.io — publishing Cargo.toml version as-is (2b)."
  cargo publish --token "$CARGO_REGISTRY_TOKEN"
  exit 0
fi

# Same major.minor (2a territory)
FULL_CMP=$(compare_full "$MAIN_M" "$MAIN_N" "$MAIN_P" "$CRATES_M" "$CRATES_N" "$CRATES_P")

if [[ "$FULL_CMP" == -1 ]]; then
  echo "error: same major.minor as crates.io but patch on main ($MAIN_VER) < published ($CRATES_VER) — refusing downgrade."
  exit 1
fi

if [[ "$FULL_CMP" == 1 ]]; then
  echo "Same major.minor; main patch ahead of crates.io — publishing $MAIN_VER (2b-style)."
  cargo publish --token "$CARGO_REGISTRY_TOKEN"
  exit 0
fi

# Full version equal: bump patch (2a)
NEW_P=$((MAIN_P + 1))
NEW_VER="${MAIN_M}.${MAIN_N}.${NEW_P}"
echo "Same version as crates.io ($MAIN_VER) — bumping patch to $NEW_VER (2a)."

sed -i.bak "s/^version = \".*\"/version = \"${NEW_VER}\"/" Cargo.toml
rm -f Cargo.toml.bak

echo "Updated Cargo.toml to version $NEW_VER"
cargo publish --token "$CARGO_REGISTRY_TOKEN"

if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  git config user.name "github-actions[bot]"
  git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
  git add Cargo.toml
  if git diff --staged --quiet; then
    echo "No staged changes (unexpected)"
    exit 1
  fi
  git commit -m "[skip ci] chore: bump version to ${NEW_VER}"
  git push origin "HEAD:${GITHUB_REF_NAME:-main}"
  echo "Pushed version bump to ${GITHUB_REF_NAME:-main}."
else
  echo "warning: GITHUB_TOKEN not set; Cargo.toml bump not pushed. Set contents: write + checkout token for auto-push."
fi

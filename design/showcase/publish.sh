#!/usr/bin/env bash
# Stage the showcase for here.now publishing (site files only — no reference
# captures, briefs, or tooling) and publish.
#
# Default: updates the existing site (slug from SHOWCASE_SLUG, falling back to
# ivory-mudra-gzs8). Minting a brand-new site requires SHOWCASE_NEW_SITE=1.
set -euo pipefail
cd "$(dirname "$0")"

SLUG="${SHOWCASE_SLUG:-ivory-mudra-gzs8}"

STAGE=".stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/shared"
cp index.html "$STAGE/"
cp shared/tokens.css shared/components.css shared/fit.js \
   shared/demo.js shared/demo.css shared/demo-host.js "$STAGE/shared/"
rsync -a --include='*/' --include='index.html' --include='*.css' --include='*.js' --include='*.svg' --include='*.woff2' --exclude='*' shots/ "$STAGE/shots/"

# Sanity gates: the landing shell plus exactly the expected scene + demo files.
scene_count=$(find "$STAGE/shots" -name index.html | wc -l | tr -d ' ')
if [[ "$scene_count" -ne 19 ]]; then
  echo "publish aborted: expected 19 scene documents, staged $scene_count" >&2
  exit 1
fi
demo_count=$(find "$STAGE/shots" -name demo.js | wc -l | tr -d ' ')
if [[ "$demo_count" -ne 19 ]]; then
  echo "publish aborted: expected 19 scene demo.js files, staged $demo_count" >&2
  exit 1
fi
if [[ -f demo-manifest.json ]]; then
  cp demo-manifest.json "$STAGE/"
  if ! python3 -c "
import json,sys
m=json.load(open('demo-manifest.json'))
ok = m.get('sceneCount')==19 and m.get('canonicalStaticHashesMatch') is True \
  and all(s.get('smokeStatus')=='pass' for s in m.get('scenes',[]))
sys.exit(0 if ok else 1)"; then
    echo "publish aborted: demo-manifest.json gates failed" >&2
    exit 1
  fi
else
  echo "publish aborted: demo-manifest.json missing (run demo-smoke.ts)" >&2
  exit 1
fi

SLUG_ARGS=(--slug "$SLUG")
if [[ "${SHOWCASE_NEW_SITE:-0}" == "1" ]]; then
  SLUG_ARGS=()
fi

exec ~/.agents/skills/here-now/scripts/publish.sh "$STAGE" --client claude-code \
  --title "Script Kit — pixel-perfect HTML tour" \
  --description "Every frame is a live HTML/CSS recreation of the real app, pixel-matched against macOS captures." \
  "${SLUG_ARGS[@]}" \
  "$@"

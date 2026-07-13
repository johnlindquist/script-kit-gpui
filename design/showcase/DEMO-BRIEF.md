# Scene demo implementation brief (interaction round)

You are adding a self-driven, explanatory demo to ONE showcase scene. The
shared machinery already exists and is frozen — do not modify anything under
`design/showcase/shared/`.

## Reference implementation (study first)

- `design/showcase/shots/02-search-filter/index.html` — hooks, hidden
  demo-only rows, `[hidden]{display:none!important}` guard, gated CSS, and the
  loader snippet (copy it verbatim; it replaces the bare fit.js line):
  ```html
  <script src="../../shared/fit.js"></script>
  <script>
  (() => {
    const params = new URLSearchParams(location.search);
    if (params.get("demo") !== "1") return;
    const runner = document.createElement("script");
    runner.src = "../../shared/demo.js";
    runner.dataset.config = "./demo.js";
    document.head.appendChild(runner);
  })();
  </script>
  ```
- `design/showcase/shots/02-search-filter/demo.js` — config shape.
- `design/showcase/shared/demo.js` — the runner. Ops: caption, keypress
  (keys[], optional activate selector → flashes data-selected="true"), pause,
  setText, typeInto (clear, perCharacterMs, optional filter{items,
  matchAttribute}), setState (attribute, value; null removes), setClass,
  moveSelection (group, to, optional state vocab override {type:"class",
  selected:"sel"}), moveNode (before/into), show, hide, filter, patch{ops},
  applyState (named state = op list), effect (name: pulse|fadeIn|fadeOut|
  waveform|thinking, target, durationMs, holdMs), loop (delayMs).
  Config: { id, initialHoldMs, idleResetMs, loopDelayMs, hudPlacement?,
  controls: {input?, list?}, states?, steps }.

## Rules

1. Only touch `design/showcase/shots/<your-id>/` (index.html, new demo.js,
   receipt.json) plus /tmp scratch.
2. Target elements with inert `data-demo-key` / `data-demo-role` /
   `data-demo-match` attributes you add to the markup. Never rely on
   :nth-child.
3. Visible state changes must reuse the scene's EXISTING state vocabulary
   (data-state="selected"/"hover", .sel, .selected, hidden, data-selected,
   textContent). Never invent new colors. If a scene-local rule is needed,
   gate it: `html[data-sk-demo="1"] …` so the canonical frame can't change.
4. Any element that must exist for the demo but not the canonical frame gets
   `hidden data-demo-only`. Add `[hidden] { display: none !important; }` to
   the scene <style> if not present.
5. Give every meaningful step an `id`; the required checkpoint id for your
   scene MUST appear on a step.
6. No bitmaps, no external assets, no network, no eval/innerHTML.
7. CSS-wide keywords (inherit/initial/unset) must NEVER appear inside a
   `font:` shorthand anywhere (known Chromium global re-raster bug — found
   the hard way).

## Mandatory verification (all three gates)

Baseline hashes: /tmp/showcase-static-baseline.sha256

```bash
ID=<your-id>
# Gate 1: canonical render unchanged
bun design/showcase/verify.ts $ID          # from repo root
shasum -a 256 .test-output/showcase-verify/$ID-render.png   # must equal baseline line
# Gate 2: paused demo pixel-identical — USE A FRESH agent-browser SESSION
# (Chromium caches file:// css/js aggressively; stale cache = false diffs)
W=<logical-w> H=<logical-h>
agent-browser --session demo-$ID-$RANDOM set viewport $W $H 2
agent-browser --session demo-$ID-$RANDOM open "file://$PWD/design/showcase/shots/$ID/index.html?demo=1&autoplay=0&hud=0"
sleep 2 && agent-browser --session demo-$ID-$RANDOM screenshot body /tmp/$ID-paused.png
magick /tmp/$ID-paused.png -crop <PW>x<PH>+0+0 +repage -colorspace sRGB -strip PNG24:/tmp/$ID-paused-n.png
magick compare -metric AE /tmp/$ID-paused-n.png .test-output/showcase-verify/$ID-render.png null:  # MUST be 0
# Gate 3: accelerated cycle clean + reset canonical
agent-browser ... open "...?demo=1&once=1&speed=6&hud=0"
# poll: window.__SK_DEMO__.status becomes "done", errors [], your checkpoint id
# appeared in document.documentElement.dataset.demoStep during the run
# then screenshot body again → AE vs canonical MUST be 0
```

If a selector is missing the runner sets `data-demo-error` on <html> and does
nothing — check `window.__SK_DEMO__.errors` first when debugging.

## Receipt

Append to your shot's receipt.json:
```json
"demo": {
  "version": 1, "script": "demo.js", "visualSimulation": true,
  "backendCapabilities": [], "checkpoint": "<checkpoint-id>",
  "autoplay": true, "userControls": [...],
  "extraCanonicalHiddenDom": [...], "canonicalRenderUnchanged": true,
  "smokeStatus": "pass"
}
```
backendCapabilities MUST stay [] — demos are visual simulations only; nothing
executes, no clipboard/mic/network/app access ever.

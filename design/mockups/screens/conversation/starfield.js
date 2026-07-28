// Deterministic Canvas 2D approximation of fx_starfield (shader id 3).
// Colors come from the generated tokens (--sk-starfield-color-a/b); geometry
// uses a fixed PRNG seed so every render is identical. Frozen by default;
// pass ?motion=animate for the gallery's live twinkle.
//
// Known divergence: this is a visual approximation, non-blocking in pixel
// diffs, until the Metal function is ported to WebGL2 (see known-divergence.json).
(() => {
  const canvas = document.getElementById("starfield");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const styles = getComputedStyle(document.documentElement);
  const colorA = styles.getPropertyValue("--sk-starfield-color-a").trim();
  const colorB = styles.getPropertyValue("--sk-starfield-color-b").trim();
  const animate = new URLSearchParams(location.search).get("motion") === "animate";

  // mulberry32 with a fixed seed — same field every load.
  const rand = (seed => () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  })(0x5c71f7);

  const W = canvas.width;
  const H = canvas.height;
  const stars = Array.from({ length: 140 }, () => ({
    x: rand() * W,
    y: rand() * H,
    r: rand() < 0.75 ? 1 : 2,
    warm: rand() < 0.35,
    phase: rand() * Math.PI * 2,
    alpha: 0.25 + rand() * 0.65,
  }));

  function draw(t) {
    ctx.clearRect(0, 0, W, H);
    for (const s of stars) {
      const twinkle = animate ? 0.6 + 0.4 * Math.sin(t / 900 + s.phase) : 1;
      ctx.globalAlpha = s.alpha * twinkle;
      ctx.fillStyle = s.warm ? colorB : colorA;
      // soft glow pass
      ctx.beginPath();
      ctx.arc(s.x, s.y, s.r * 2.2, 0, Math.PI * 2);
      ctx.globalAlpha *= 0.25;
      ctx.fill();
      // sharp core pass
      ctx.globalAlpha = s.alpha * twinkle;
      ctx.beginPath();
      ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalAlpha = 1;
    if (animate) requestAnimationFrame(draw);
  }
  draw(0);
})();

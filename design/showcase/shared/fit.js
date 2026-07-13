/* Scale the fixed-size .scene to fill the embedding iframe exactly,
 * centering any sub-pixel residual instead of leaving a right/bottom gutter.
 * At exact-size verifier viewports this resolves to translate(0,0) scale(1). */
(function () {
  "use strict";
  function fit() {
    var scene = document.querySelector(".scene");
    if (!scene) return;
    var sw = scene.offsetWidth;
    var sh = scene.offsetHeight;
    var vw = window.innerWidth;
    var vh = window.innerHeight;
    if (!sw || !sh || !vw || !vh) return;
    var scale = Math.min(vw / sw, vh / sh);
    var x = (vw - sw * scale) / 2;
    var y = (vh - sh * scale) / 2;
    scene.style.transformOrigin = "0 0";
    scene.style.transform =
      "translate3d(" + x + "px, " + y + "px, 0) scale(" + scale + ")";
  }
  window.addEventListener("resize", fit);
  window.addEventListener("load", fit);
  fit();
})();

/* Landing-page demo host: owns iframe demo scheduling (parent-side
 * IntersectionObserver with hysteresis), the caption rail under each embed,
 * click-to-engage, wheel forwarding, and reduced-motion behavior.
 *
 * Scenes are embedded with ?demo=1&host=landing&autoplay=0 and stay inert
 * (pointer-events:none) until engaged via the rail's Explore button.
 */
(function () {
  "use strict";

  var CHANNEL = "sk-showcase-demo";
  var REDUCED = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var MAX_RUNNING = 3;
  var START_RATIO = 0.55;
  var STOP_RATIO = 0.15;

  var entries = []; // {iframe, figure, rail, caption, keys, engageBtn, ratio, running, ready, offscreenSince, startedOnce}
  var engaged = null;

  function post(entry, msg) {
    if (!entry.iframe.contentWindow) return;
    msg.channel = CHANNEL;
    msg.version = 1;
    entry.iframe.contentWindow.postMessage(msg, "*");
  }

  function buildRail(iframe) {
    var figure = iframe.closest("figure") || iframe.parentElement;
    var rail = document.createElement("div");
    rail.className = "sk-demo-rail";
    rail.setAttribute("aria-live", "polite");
    var caption = document.createElement("span");
    caption.className = "sk-demo-caption";
    var keys = document.createElement("span");
    keys.className = "sk-demo-keys";
    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "sk-demo-engage";
    btn.textContent = REDUCED ? "Play demo" : "Explore";
    rail.appendChild(caption);
    rail.appendChild(keys);
    rail.appendChild(btn);
    // after the iframe, before any existing figcaption
    var figcap = figure.querySelector("figcaption");
    if (figcap) figure.insertBefore(rail, figcap);
    else figure.appendChild(rail);
    return { figure: figure, rail: rail, caption: caption, keys: keys, engageBtn: btn };
  }

  function release(entry, refocus) {
    if (engaged !== entry) return;
    engaged = null;
    entry.figure.removeAttribute("data-demo-engaged");
    entry.engageBtn.textContent = REDUCED ? "Play demo" : "Explore";
    post(entry, { type: "release" });
    if (refocus) entry.engageBtn.focus();
  }

  function engage(entry) {
    if (engaged && engaged !== entry) release(engaged, false);
    engaged = entry;
    entry.figure.setAttribute("data-demo-engaged", "1");
    entry.engageBtn.textContent = "Esc releases";
    post(entry, { type: "engage" });
    entry.iframe.focus();
    try { entry.iframe.contentWindow.focus(); } catch (_) {}
  }

  function schedule() {
    var eligible = entries
      .filter(function (e) { return e.ready && e.ratio >= START_RATIO; })
      .sort(function (a, b) { return b.ratio - a.ratio; });
    var stopping = entries.filter(function (e) { return e.running && e.ratio < STOP_RATIO; });

    stopping.forEach(function (e) {
      e.running = false;
      post(e, { type: "active", value: false });
      e.offscreenSince = Date.now();
      setTimeout(function () {
        if (!e.running && e.offscreenSince && Date.now() - e.offscreenSince >= 2000) {
          post(e, { type: "reset" });
        }
      }, 2100);
    });

    var running = entries.filter(function (e) { return e.running; }).length;
    var delay = 0;
    eligible.forEach(function (e) {
      if (e.running || running >= MAX_RUNNING || REDUCED) return;
      running += 1;
      e.running = true;
      e.offscreenSince = null;
      e.startedOnce = true;
      setTimeout(function () {
        if (e.running) post(e, { type: "active", value: true });
      }, delay);
      delay += 600; // stagger sibling starts
    });
  }

  var debounceTimer = null;
  function debouncedSchedule() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(schedule, 250);
  }

  function init() {
    var iframes = Array.prototype.slice.call(document.querySelectorAll("iframe.embed"));
    iframes.forEach(function (iframe) {
      iframe.style.pointerEvents = "none";
      var ui = buildRail(iframe);
      var entry = {
        iframe: iframe, figure: ui.figure, rail: ui.rail, caption: ui.caption,
        keys: ui.keys, engageBtn: ui.engageBtn, ratio: 0, running: false,
        ready: false, offscreenSince: null, startedOnce: false,
      };
      ui.engageBtn.addEventListener("click", function () {
        if (REDUCED) { post(entry, { type: "replay" }); return; }
        if (engaged === entry) release(entry, true);
        else engage(entry);
      });
      entries.push(entry);
    });

    var io = new IntersectionObserver(function (obs) {
      obs.forEach(function (rec) {
        var entry = entries.find(function (e) { return e.iframe === rec.target; });
        if (entry) entry.ratio = rec.intersectionRatio;
      });
      debouncedSchedule();
    }, { threshold: [0, STOP_RATIO, 0.35, START_RATIO, 0.75, 1] });
    entries.forEach(function (e) { io.observe(e.iframe); });

    window.addEventListener("message", function (ev) {
      var d = ev.data;
      if (!d || d.channel !== CHANNEL) return;
      var entry = entries.find(function (e) {
        return e.iframe.contentWindow === ev.source;
      });
      if (!entry) return;
      if (d.type === "ready") {
        entry.ready = true;
        debouncedSchedule();
      } else if (d.type === "caption") {
        entry.caption.textContent = d.text;
      } else if (d.type === "keypress") {
        entry.keys.textContent = "";
        (d.keys || []).forEach(function (k) {
          var chip = document.createElement("kbd");
          chip.className = "sk-demo-key";
          chip.textContent = k;
          entry.keys.appendChild(chip);
        });
        setTimeout(function () { entry.keys.textContent = ""; }, 1400);
      } else if (d.type === "wheel") {
        window.scrollBy({ top: d.deltaY, left: d.deltaX });
      } else if (d.type === "error") {
        entry.caption.textContent = "";
        entry.rail.dataset.error = "1";
      }
    });

    document.addEventListener("keydown", function (ev) {
      if (ev.key === "Escape" && engaged) release(engaged, true);
    });
    document.addEventListener("visibilitychange", function () {
      if (document.hidden) {
        entries.forEach(function (e) {
          if (e.running) { e.running = false; post(e, { type: "active", value: false }); }
        });
      } else debouncedSchedule();
    });
    window.addEventListener("pagehide", function () {
      entries.forEach(function (e) {
        if (e.running) post(e, { type: "active", value: false });
      });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else init();
})();

/* Script Kit showcase — shared demo runner.
 *
 * Loaded ONLY behind the strict ?demo=1 gate (see the per-scene loader
 * snippet). Executes a small, closed, declarative op set over the scene's
 * existing state vocabulary (data-state, hidden, textContent, …), recording
 * every mutation in a ledger so reset restores the pixel-canonical frame
 * exactly. No eval, no innerHTML, no scene-supplied callbacks or timers.
 *
 * URL params: demo=1 (gate) autoplay=0|1 host=landing once=1 speed=N hud=0
 * Messages (channel "sk-showcase-demo" v1):
 *   scene → host: ready | caption | keypress | state | error | wheel
 *   host → scene: active | engage | release | replay | reset
 */
(function () {
  "use strict";

  var params = new URLSearchParams(location.search);
  var HOSTED = params.get("host") === "landing";
  var AUTOPLAY = params.get("autoplay") !== "0";
  var ONCE = params.get("once") === "1";
  var SPEED = Math.max(0.001, Number(params.get("speed") || "1"));
  var HUD_ON = params.get("hud") !== "0" && !HOSTED;
  var REDUCED = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  var CHANNEL = "sk-showcase-demo";
  var VERSION = 1;

  var api = {
    id: null,
    status: "init",
    stepId: null,
    seenSteps: [],
    cycle: 0,
    running: false,
    engaged: false,
    errors: [],
  };
  window.__SK_DEMO__ = api;

  function post(type, extra) {
    if (!HOSTED || window.parent === window) return;
    var msg = { channel: CHANNEL, version: VERSION, type: type, sceneId: api.id };
    if (extra) for (var k in extra) msg[k] = extra[k];
    window.parent.postMessage(msg, "*");
  }

  function fail(err) {
    api.status = "error";
    api.errors.push(String(err));
    document.documentElement.dataset.demoError = String(err);
    post("error", { error: String(err) });
  }

  /* ---------------- mutation ledger ---------------- */

  var ledger = new Map();
  function entry(el) {
    var e = ledger.get(el);
    if (!e) {
      e = { attrs: new Map(), text: null, hidden: null, moved: null, style: new Map() };
      ledger.set(el, e);
    }
    return e;
  }
  function recAttr(el, name) {
    var e = entry(el);
    if (!e.attrs.has(name)) e.attrs.set(name, el.getAttribute(name));
  }
  function recText(el) {
    var e = entry(el);
    if (e.text === null) e.text = { value: el.textContent };
  }
  function recHidden(el) {
    var e = entry(el);
    if (e.hidden === null) e.hidden = { value: el.hidden };
  }
  function recMove(el) {
    var e = entry(el);
    if (!e.moved) e.moved = { parent: el.parentNode, next: el.nextSibling };
  }
  function recStyle(el, prop) {
    var e = entry(el);
    if (!e.style.has(prop)) e.style.set(prop, el.style.getPropertyValue(prop));
  }

  var liveAnimations = [];
  function cancelAnimations() {
    liveAnimations.forEach(function (a) { try { a.cancel(); } catch (_) {} });
    liveAnimations = [];
  }

  function resetLedger() {
    cancelAnimations();
    return new Promise(function (resolve) {
      requestAnimationFrame(function () {
        ledger.forEach(function (e, el) {
          e.attrs.forEach(function (v, name) {
            if (v === null) el.removeAttribute(name);
            else el.setAttribute(name, v);
          });
          if (e.text) el.textContent = e.text.value;
          if (e.hidden) el.hidden = e.hidden.value;
          e.style.forEach(function (v, prop) {
            if (v) el.style.setProperty(prop, v);
            else el.style.removeProperty(prop);
          });
          if (e.moved) e.moved.parent.insertBefore(el, e.moved.next);
        });
        ledger.clear();
        delete document.documentElement.dataset.demoStep;
        api.stepId = null;
        resolve();
      });
    });
  }

  /* ---------------- HUD ---------------- */

  var hud = null, hudCaption = null, hudKeys = null;
  function ensureHud(placement) {
    if (!HUD_ON || hud) return;
    hud = document.createElement("div");
    hud.className = "sk-demo-hud";
    if (placement) hud.dataset.placement = placement;
    hudCaption = document.createElement("span");
    hudCaption.className = "sk-demo-hud__caption";
    hudKeys = document.createElement("span");
    hudKeys.className = "sk-demo-hud__keys";
    hud.appendChild(hudCaption);
    hud.appendChild(hudKeys);
    document.body.appendChild(hud);
  }
  function showCaption(text) {
    post("caption", { text: text });
    if (hudCaption) {
      hudCaption.textContent = text;
      hud.dataset.visible = "1";
    }
  }
  function showKeys(keys) {
    post("keypress", { keys: keys });
    if (hudKeys) {
      hudKeys.textContent = "";
      keys.forEach(function (k) {
        var chip = document.createElement("kbd");
        chip.className = "sk-demo-key";
        chip.textContent = k;
        hudKeys.appendChild(chip);
      });
      hud.dataset.visible = "1";
    }
  }
  function clearKeys() {
    if (hudKeys) hudKeys.textContent = "";
  }

  /* ---------------- op helpers ---------------- */

  function q(sel) {
    var el = document.querySelector(sel);
    if (!el) throw new Error("selector not found: " + sel);
    return el;
  }
  function qa(sel) {
    return Array.prototype.slice.call(document.querySelectorAll(sel));
  }

  function ms(v) { return Math.max(0, (v || 0) / SPEED); }

  var currentAbort = null;
  function wait(duration) {
    return new Promise(function (resolve, reject) {
      var t = setTimeout(resolve, ms(duration));
      currentAbort.signal.addEventListener("abort", function () {
        clearTimeout(t);
        reject(new DOMException("aborted", "AbortError"));
      }, { once: true });
    });
  }

  function stateVocab(cfg) {
    var list = (cfg.controls && cfg.controls.list && cfg.controls.list.state) || {
      type: "attribute", name: "data-state", selected: "selected", hover: "hover",
    };
    return list;
  }

  function setSelectionState(el, vocab, kind, on) {
    if (vocab.type === "class") {
      recAttr(el, "class");
      el.classList[on ? "add" : "remove"](vocab[kind]);
    } else {
      recAttr(el, vocab.name);
      if (on) el.setAttribute(vocab.name, vocab[kind]);
      else el.removeAttribute(vocab.name);
    }
  }

  function applyFilter(itemsSel, matchAttr, query) {
    var qy = query.trim().toLowerCase();
    qa(itemsSel).forEach(function (el) {
      recHidden(el);
      if (!qy) { el.hidden = el.hasAttribute("data-demo-only"); return; }
      var tokens = (el.getAttribute(matchAttr) || "").toLowerCase().split(/\s+/);
      var match = tokens.some(function (t) { return t.indexOf(qy) === 0; });
      el.hidden = !match;
    });
  }

  var EFFECTS = {
    pulse: function (el, d) {
      return el.animate(
        [{ opacity: 1 }, { opacity: 0.35 }, { opacity: 1 }],
        { duration: ms(d || 600), iterations: 2 }
      );
    },
    fadeIn: function (el, d) {
      recStyle(el, "opacity");
      el.style.opacity = "1";
      return el.animate([{ opacity: 0 }, { opacity: 1 }], { duration: ms(d || 220) });
    },
    fadeOut: function (el, d) {
      recStyle(el, "opacity");
      el.style.opacity = "0";
      return el.animate([{ opacity: 1 }, { opacity: 0 }], { duration: ms(d || 220) });
    },
    waveform: function (el, d) {
      var bars = el.children.length ? Array.prototype.slice.call(el.children) : [el];
      var seed = 7;
      bars.forEach(function (bar, i) {
        seed = (seed * 31 + i * 17) % 97;
        var s1 = 0.4 + (seed % 55) / 100;
        var s2 = 0.5 + ((seed * 13) % 45) / 100;
        var a = bar.animate(
          [{ transform: "scaleY(1)" }, { transform: "scaleY(" + s1 + ")" },
           { transform: "scaleY(" + s2 + ")" }, { transform: "scaleY(1)" }],
          { duration: ms(700 + (seed % 5) * 90), iterations: Math.ceil(ms(d || 2000) / 700) }
        );
        liveAnimations.push(a);
      });
      return null;
    },
    thinking: function (el, d) {
      return el.animate(
        [{ opacity: 1 }, { opacity: 0.45 }, { opacity: 1 }],
        { duration: ms(d || 900), iterations: Math.max(1, Math.round(ms(d || 1800) / 900)) }
      );
    },
  };

  /* ---------------- ops ---------------- */

  function makeOps(cfg) {
    var vocab = stateVocab(cfg);
    var ops = {
      caption: function (s) { showCaption(s.text); return wait(s.holdMs || 1100); },
      pause: function (s) { return wait(s.ms || s.holdMs || 400); },
      keypress: function (s) {
        showKeys(s.keys);
        var target = null;
        if (s.activate) {
          target = q(s.activate);
          recAttr(target, "data-selected");
          target.setAttribute("data-selected", "true");
        }
        return wait(s.holdMs || 700).then(function () {
          clearKeys();
          if (target && !s.persist) {
            target.removeAttribute("data-selected");
            var orig = ledger.get(target);
            if (orig) {
              var v = orig.attrs.get("data-selected");
              if (v !== null && v !== undefined) target.setAttribute("data-selected", v);
            }
          }
        });
      },
      setText: function (s) {
        var el = q(s.target);
        recText(el);
        el.textContent = s.text;
      },
      typeInto: function (s) {
        var el = q(s.target);
        recText(el);
        recAttr(el, "data-state");
        el.setAttribute("data-state", "input");
        var text = s.text;
        if (REDUCED) {
          el.textContent = s.clear ? text : el.textContent + text;
          if (s.filter) applyFilter(s.filter.items, s.filter.matchAttribute, text);
          return wait(300);
        }
        if (s.clear) el.textContent = "";
        var chain = Promise.resolve();
        text.split("").forEach(function (ch, i) {
          chain = chain.then(function () {
            el.textContent += ch;
            if (s.filter) {
              applyFilter(s.filter.items, s.filter.matchAttribute, text.slice(0, i + 1));
            }
            return wait(s.perCharacterMs || 70);
          });
        });
        return chain;
      },
      setState: function (s) {
        var el = q(s.target);
        recAttr(el, s.attribute);
        if (s.value === null) el.removeAttribute(s.attribute);
        else el.setAttribute(s.attribute, s.value);
      },
      setClass: function (s) {
        var el = q(s.target);
        recAttr(el, "class");
        el.classList[s.on === false ? "remove" : "add"](s.className);
      },
      moveSelection: function (s) {
        var group = qa(s.group).filter(function (el) { return !el.hidden; });
        var vv = s.state || vocab;
        group.forEach(function (el) {
          var has = vv.type === "class"
            ? el.classList.contains(vv.selected)
            : el.getAttribute(vv.name) === vv.selected;
          if (has) setSelectionState(el, vv, "selected", false);
        });
        setSelectionState(q(s.to), vv, "selected", true);
        return wait(s.holdMs || 0);
      },
      moveNode: function (s) {
        var el = q(s.target);
        recMove(el);
        var ref = q(s.before || s.into);
        if (s.before) ref.parentNode.insertBefore(el, ref);
        else ref.appendChild(el);
      },
      show: function (s) {
        qa(s.target).forEach(function (el) { recHidden(el); el.hidden = false; });
      },
      hide: function (s) {
        qa(s.target).forEach(function (el) { recHidden(el); el.hidden = true; });
      },
      filter: function (s) { applyFilter(s.items, s.matchAttribute, s.query || ""); },
      patch: function (s) {
        return new Promise(function (resolve) {
          requestAnimationFrame(function () {
            (s.ops || []).forEach(function (sub) {
              var fn = ops[sub.op];
              if (!fn) throw new Error("unknown patch op: " + sub.op);
              fn(sub);
            });
            resolve();
          });
        });
      },
      applyState: function (s) {
        var list = cfg.states && cfg.states[s.name];
        if (!list) throw new Error("unknown state: " + s.name);
        var chain = Promise.resolve();
        list.forEach(function (sub) {
          chain = chain.then(function () { return ops[sub.op](sub); });
        });
        return chain;
      },
      effect: function (s) {
        if (REDUCED) return wait(s.holdMs || 0);
        var el = q(s.target);
        var eff = EFFECTS[s.name];
        if (!eff) throw new Error("unknown effect: " + s.name);
        var a = eff(el, s.durationMs);
        if (a) liveAnimations.push(a);
        return wait(s.holdMs !== undefined ? s.holdMs : (s.durationMs || 600));
      },
      loop: function () { return "loop"; },
    };
    return ops;
  }

  /* ---------------- selector validation ---------------- */

  function collectSelectors(steps, out) {
    (steps || []).forEach(function (s) {
      ["target", "group", "to", "before", "into", "activate"].forEach(function (k) {
        if (s[k]) out.push(s[k]);
      });
      if (s.filter) out.push(s.filter.items);
      if (s.ops) collectSelectors(s.ops, out);
    });
  }

  function validate(cfg) {
    var sels = [];
    collectSelectors(cfg.steps, sels);
    Object.keys(cfg.states || {}).forEach(function (name) {
      collectSelectors(cfg.states[name], sels);
    });
    var missing = sels.filter(function (sel) {
      try { return !document.querySelector(sel); } catch (e) { return true; }
    });
    if (missing.length) {
      throw new Error("demo selectors missing: " + missing.join(", "));
    }
  }

  /* ---------------- run loop ---------------- */

  var cfg = null;
  var idleTimer = null;
  var exploring = false;

  function runCycle() {
    var ops = makeOps(cfg);
    api.cycle += 1;
    api.running = true;
    api.status = "running";
    document.documentElement.dataset.demoCycle = String(api.cycle);
    currentAbort = new AbortController();

    var chain = wait(cfg.initialHoldMs !== undefined ? cfg.initialHoldMs : 900);
    var looped = false;
    (cfg.steps || []).forEach(function (step) {
      chain = chain.then(function () {
        if (looped) return;
        if (step.op === "loop") {
          looped = true;
          return wait(step.delayMs !== undefined ? step.delayMs : (cfg.loopDelayMs || 1200));
        }
        var fn = ops[step.op];
        if (!fn) throw new Error("unknown op: " + step.op);
        var r = fn(step);
        if (step.id) {
          api.stepId = step.id;
          api.seenSteps.push(step.id);
          document.documentElement.dataset.demoStep = step.id;
          post("state", { stepId: step.id });
        }
        return r;
      });
    });
    return chain.then(function () {
      api.running = false;
      return resetLedger().then(function () {
        if (ONCE || REDUCED) {
          api.status = "done";
          post("state", { stepId: "done" });
        } else if (!exploring) {
          return wait(600).then(runCycle);
        }
      });
    }).catch(function (err) {
      api.running = false;
      if (err && err.name === "AbortError") return;
      fail(err);
      return resetLedger();
    });
  }

  function stopAutoplay() {
    if (currentAbort) currentAbort.abort();
    cancelAnimations();
    api.running = false;
  }

  function scheduleIdleReset() {
    clearTimeout(idleTimer);
    idleTimer = setTimeout(function () {
      showCaption("Demo restarting.");
      resetLedger().then(function () {
        exploring = false;
        if (!REDUCED) return wait0(600).then(runCycle);
        api.status = "ready";
      });
    }, ms(cfg.idleResetMs || 8000));
  }
  function wait0(d) {
    return new Promise(function (r) { setTimeout(r, ms(d)); });
  }

  /* ---------------- exploration ---------------- */

  function beginExploring() {
    if (!exploring) {
      exploring = true;
      stopAutoplay();
      api.status = "exploring";
      post("state", { stepId: "exploring" });
      showCaption("You’re exploring.");
    }
    scheduleIdleReset();
  }

  function visibleItems(listCfg) {
    return qa(listCfg.items).filter(function (el) { return !el.hidden; });
  }

  function wireExploration() {
    var controls = cfg.controls || {};
    var vocab = stateVocab(cfg);
    var interactive = function () { return api.engaged || !HOSTED; };

    if (controls.list) {
      document.addEventListener("pointermove", function (ev) {
        if (!interactive()) return;
        var row = ev.target.closest && ev.target.closest(controls.list.items);
        if (!row || row.hidden) return;
        beginExploring();
        var isSelected = vocab.type === "class"
          ? row.classList.contains(vocab.selected)
          : row.getAttribute(vocab.name) === vocab.selected;
        if (isSelected) return;
        visibleItems(controls.list).forEach(function (el) {
          var hovered = vocab.type === "class"
            ? el.classList.contains(vocab.hover)
            : el.getAttribute(vocab.name) === vocab.hover;
          if (hovered && el !== row) setSelectionState(el, vocab, "hover", false);
        });
        setSelectionState(row, vocab, "hover", true);
      }, { passive: true });

      document.addEventListener("click", function (ev) {
        if (!interactive()) return;
        var row = ev.target.closest && ev.target.closest(controls.list.items);
        if (!row || row.hidden) return;
        beginExploring();
        visibleItems(controls.list).forEach(function (el) {
          setSelectionState(el, vocab, "selected", false);
        });
        setSelectionState(row, vocab, "selected", true);
      });
    }

    document.addEventListener("keydown", function (ev) {
      if (!interactive()) return;
      if (ev.key === "Escape") {
        post("state", { stepId: "released" });
        resetLedger().then(function () {
          exploring = false;
          clearTimeout(idleTimer);
          if (!REDUCED && !ONCE) runCycle();
        });
        return;
      }
      beginExploring();
      if (controls.list && (ev.key === "ArrowDown" || ev.key === "ArrowUp")) {
        ev.preventDefault();
        var items = visibleItems(controls.list);
        var idx = items.findIndex(function (el) {
          return vocab.type === "class"
            ? el.classList.contains(vocab.selected)
            : el.getAttribute(vocab.name) === vocab.selected;
        });
        var next = ev.key === "ArrowDown" ? Math.min(items.length - 1, idx + 1) : Math.max(0, idx - 1);
        if (idx >= 0 && items[idx] !== items[next]) setSelectionState(items[idx], vocab, "selected", false);
        if (items[next]) setSelectionState(items[next], vocab, "selected", true);
      } else if (controls.input && (ev.key.length === 1 || ev.key === "Backspace") && !ev.metaKey && !ev.ctrlKey) {
        ev.preventDefault();
        var input = q(controls.input.target);
        recText(input);
        recAttr(input, "data-state");
        input.setAttribute("data-state", "input");
        var cur = input.textContent;
        if (ev.key === "Backspace") cur = cur.slice(0, -1);
        else if (cur.length < (controls.input.maxLength || 40)) cur += ev.key;
        input.textContent = cur;
        if (controls.input.items) {
          applyFilter(controls.input.items, controls.input.matchAttribute || "data-demo-match", cur);
        }
      }
    });

    // Wheel forwarding so an engaged iframe never traps page scroll.
    if (HOSTED) {
      window.addEventListener("wheel", function (ev) {
        if (ev.ctrlKey) return;
        ev.preventDefault();
        post("wheel", { deltaY: ev.deltaY, deltaX: ev.deltaX });
      }, { passive: false });
    }
  }

  /* ---------------- host messages ---------------- */

  var active = !HOSTED;
  function wireHost() {
    window.addEventListener("message", function (ev) {
      var d = ev.data;
      if (!d || d.channel !== CHANNEL) return;
      if (ev.source !== window.parent) return;
      if (d.type === "active") {
        var was = active;
        active = !!d.value;
        if (!active && was) stopAutoplay();
        if (active && !was && !exploring && !REDUCED && !api.running && api.status !== "error") {
          runCycle();
        }
      } else if (d.type === "engage") {
        api.engaged = true;
        beginExploring();
      } else if (d.type === "release") {
        api.engaged = false;
        resetLedger().then(function () {
          exploring = false;
          clearTimeout(idleTimer);
          if (active && !REDUCED && !ONCE) runCycle();
        });
      } else if (d.type === "replay" || d.type === "reset") {
        stopAutoplay();
        resetLedger().then(function () {
          exploring = false;
          if (d.type === "replay" && !api.running) runCycle();
        });
      }
    });
    document.addEventListener("visibilitychange", function () {
      if (document.hidden) stopAutoplay();
    });
  }

  /* ---------------- define ---------------- */

  window.SKDemo = {
    define: function (config) {
      cfg = config;
      api.id = config.id;
      try {
        validate(config);
      } catch (err) {
        fail(err);
        return;
      }
      document.documentElement.dataset.skDemo = "1";
      ensureHud(config.hudPlacement);
      wireExploration();
      wireHost();
      api.status = "ready";
      post("ready", { reducedMotion: REDUCED });
      if (REDUCED) return; // manual play only (host sends replay)
      if (AUTOPLAY && active) runCycle();
    },
  };

  // Load the per-scene config declared by the loader.
  var self = document.currentScript;
  var configSrc = self && self.dataset.config;
  if (HUD_ON) {
    var css = document.createElement("link");
    css.rel = "stylesheet";
    css.href = "../../shared/demo.css";
    document.head.appendChild(css);
  }
  if (configSrc) {
    var s = document.createElement("script");
    s.src = configSrc;
    s.onerror = function () { fail("demo config failed to load: " + configSrc); };
    document.head.appendChild(s);
  }
})();

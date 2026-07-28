/**
 * Continuous timeline StoryPlayer (Oracle story-mockup-realism-v2).
 * rAF clock + declarative actions; seek is deterministic from t=0.
 *
 * API:
 *   StoryPlayer.defineStory(def)
 *   StoryPlayer.mount({ root, story })
 *   window.__SK_STORY__ = { play, pause, restart, seek, getTime, getState, getSemanticDigest }
 */
(function (global) {
  "use strict";

  function clamp(n, a, b) {
    return Math.max(a, Math.min(b, n));
  }

  function typePrefix(text, progress) {
    if (!text) return "";
    progress = clamp(progress, 0, 1);
    if (progress <= 0) return "";
    if (progress >= 1) return text;
    // ceil so the first frames already show a character (caret visibly advances)
    var n = Math.max(1, Math.ceil(text.length * progress));
    return text.slice(0, Math.min(n, text.length));
  }

  /** Ordered, keyed message upsert. Never appends a duplicate id, so replaying
   *  the same action list is idempotent and seek(t) == play-through-to(t). */
  function upsertMessage(sem, msg) {
    if (!sem.messages) sem.messages = [];
    var i;
    for (i = 0; i < sem.messages.length; i++) {
      if (sem.messages[i].id === msg.id) {
        sem.messages[i] = Object.assign({}, sem.messages[i], msg);
        return sem.messages[i];
      }
    }
    sem.messages.push(msg);
    return msg;
  }

  function reduce(story, t) {
    var state = {
      t: t,
      surfaces: {},
      visible: {},
      overlays: {},
      semantic: {},
    };
    (story.surfaces || []).forEach(function (s) {
      state.surfaces[s.id] = { id: s.id, role: s.role || "window" };
      state.visible[s.id] = !!s.initial;
    });
    // default first surface visible
    if (story.surfaces && story.surfaces.length && !story.surfaces.some(function (s) { return s.initial; })) {
      state.visible[story.surfaces[0].id] = true;
    }

    var actions = (story.actions || []).slice().sort(function (a, b) {
      return (a.at || 0) - (b.at || 0);
    });

    actions.forEach(function (action) {
      var at = action.at || 0;
      var dur = action.duration || 0;
      if (t < at) return;
      var progress = dur <= 0 ? 1 : clamp((t - at) / dur, 0, 1);
      var sid = action.surface;
      if (!state.semantic[sid]) state.semantic[sid] = {};
      var sem = state.semantic[sid];

      switch (action.kind) {
        case "showSurface":
          state.visible[sid] = true;
          break;
        case "hideSurface":
          state.visible[sid] = false;
          break;
        case "openOverlay":
          state.overlays[sid] = true;
          state.visible[sid] = true;
          break;
        case "closeOverlay":
          state.overlays[sid] = false;
          state.visible[sid] = false;
          break;
        case "ensureRows":
          sem.ensureRows = action.rows || action.names || [];
          break;
        case "type":
          {
            var typedText = typePrefix(action.text, progress);
            if (action.as === "composer") sem.composer = typedText;
            else if (action.as === "input") {
              sem.input = typedText;
              sem.search = typedText;
            } else if (action.as === "notes") sem.text = typedText;
            else if (action.as === "terminal") {
              sem.lines = (action.prefixLines || []).concat([
                { text: (action.prompt || "$ ") + typedText, cursor: progress < 1 },
              ]);
            } else {
              // default + filter: search box shows typed text AND list filters live
              sem.search = typedText;
              sem.filter = typedText;
            }
          }
          break;
        case "setText":
          if (action.as === "notes") sem.text = action.text;
          else if (action.as === "composer") sem.composer = action.text;
          else if (action.as === "input") sem.input = action.text;
          else {
            sem.search = action.text;
            if (action.filter) sem.filter = action.text;
          }
          break;
        case "setSelection":
          sem.selectedIndex = action.index;
          if (action.preview != null) sem.preview = action.preview;
          break;
        case "walkSelection":
          {
            var from = action.from || 0;
            var to = action.to || 0;
            var idx = Math.round(from + (to - from) * progress);
            sem.selectedIndex = idx;
            if (action.previews && action.previews[idx] != null) {
              sem.preview = action.previews[idx];
            }
          }
          break;
        case "setFooterState":
          sem.footer = action.footer;
          break;
        case "setLines":
          // progressive line reveal for day page
          if (action.mode === "typeLast" && action.lines) {
            var base = action.lines.slice(0, -1);
            var last = action.lines[action.lines.length - 1] || "";
            base.push(typePrefix(last, progress));
            sem.lines = base;
          } else {
            sem.lines = action.lines;
          }
          break;
        case "streamText":
          {
            var partial = typePrefix(action.text, progress);
            var streamId = action.msgId || "stream";
            // Update the ADDRESSED message rather than parallel slots, so the
            // full transcript is recoverable from state alone at any t.
            upsertMessage(sem, {
              id: streamId,
              role: "assistant",
              text: partial,
              state: progress < 1 ? "streaming" : "complete",
            });
            // Legacy projections (DR1) — derived, never authoritative.
            sem.streamText = partial;
            sem.streamMsgId = streamId;
            sem.streamPhase = progress < 1 ? "partial" : "completed";
            if (progress > 0) sem.streamStarted = true;
          }
          break;
        case "appendMessage":
          {
            var msgId = action.msgId || action.role + "-" + at;
            upsertMessage(sem, {
              id: msgId,
              role: action.role,
              text: action.text || "",
              state: "complete",
            });
            // Legacy projection (DR1). The adapter no longer consumes this.
            sem.appendMessage = { role: action.role, text: action.text, id: msgId };
          }
          break;
        // ── Idempotent set* kinds (schema v3). Replacement semantics only —
        // never add*/remove* — so any seek order yields the same state. ──
        case "setConversationMode":
          sem.mode = action.mode;
          sem.identity = action.identity || null;
          sem.contextPolicy = action.contextPolicy || "none";
          break;
        case "setTurnState":
          upsertMessage(sem, {
            id: action.turnId,
            state: action.state,
            statusLabel: action.statusLabel || null,
            failure: action.failure || null,
          });
          break;
        case "setQueuedTurn":
          sem.queuedTurn = action.turn || null;
          break;
        case "setContextGrants":
          sem.grants = action.grants || [];
          break;
        case "setContextReceipt":
          sem.receipt = action.receipt || null;
          break;
        case "setListRows":
          sem.sections = action.sections || [];
          break;
        case "setNotice":
          sem.notice = action.notice || null;
          break;
        case "setDocumentState":
          sem.document = {
            title: action.title,
            markdown: action.markdown,
            dirty: !!action.dirty,
            selection: action.selection || null,
            insertion: action.insertion || null,
            undoDepth: action.undoDepth || 0,
          };
          break;
        case "setArtifactState":
          sem.artifact = action.artifact || null;
          break;
        case "setSendState":
          sem.sendState = action.value;
          break;
        case "setTerminalLines":
          if (action.mode === "typeCommand") {
            var pre = action.prefixLines || [];
            var cmd = typePrefix(action.command || "", progress);
            sem.lines = pre.concat([{ text: (action.prompt || "$ ") + cmd, cursor: progress < 1 }]);
          } else {
            sem.lines = action.lines;
          }
          break;
        case "pressKey":
          // Evidence + keystroke visualization ONLY. `outcome` is a label the
          // story author asserts; the player must never infer product routing
          // from it. The next explicit action produces the state change.
          sem.lastKey = action.key;
          sem.lastKeyModifiers = action.modifiers || [];
          sem.lastKeyOutcome = action.outcome || null;
          break;
        case "pause":
          break;
        default:
          break;
      }
    });

    return state;
  }

  function mount(options) {
    var root = options.root || document.body;
    var story = options.story;
    if (!story) throw new Error("StoryPlayer.mount requires story");
    var duration = story.durationMs || 8000;
    var playing = false;
    var t = 0;
    var lastFrame = null;
    var raf = null;
    var frames = root.querySelectorAll("[data-story-surface]");
    var frameMap = {};
    frames.forEach(function (el) {
      frameMap[el.getAttribute("data-story-surface")] = el;
    });
    var ready = {};
    var labelEl = root.querySelector("[data-story-step-label]");
    var rail = root.querySelector("[data-story-rail]");
    var playBtn = root.querySelector("[data-story-play]");
    var params = new URLSearchParams(location.search);
    var autoplay = options.autoplay != null ? options.autoplay : params.get("autoplay") !== "0";
    if (params.get("t")) t = Number(params.get("t")) || 0;

    function waitReady() {
      return Promise.all(
        Object.keys(frameMap).map(function (id) {
          var iframe = frameMap[id];
          return new Promise(function (resolve) {
            function done() {
              ready[id] = true;
              resolve();
            }
            if (iframe.tagName !== "IFRAME") return done();
            if (iframe.contentDocument && iframe.contentDocument.readyState === "complete") {
              // small delay for embed script
              setTimeout(done, 30);
              return;
            }
            iframe.addEventListener("load", function () {
              setTimeout(done, 30);
            });
          });
        }),
      );
    }

    function docFor(id) {
      var iframe = frameMap[id];
      if (!iframe) return null;
      if (iframe.tagName === "IFRAME") {
        try {
          return iframe.contentDocument;
        } catch (_) {
          return null;
        }
      }
      return iframe.ownerDocument;
    }

    function applyState(state) {
      // visibility
      Object.keys(frameMap).forEach(function (id) {
        var el = frameMap[id];
        var vis = !!state.visible[id];
        el.hidden = !vis;
        el.style.visibility = vis ? "visible" : "hidden";
        el.style.pointerEvents = vis ? "auto" : "none";
        if (state.overlays[id]) el.setAttribute("data-overlay", "true");
        else el.removeAttribute("data-overlay");
      });

      var adapters = (global.StorySurfaces && global.StorySurfaces.adapters) || {};
      Object.keys(state.semantic).forEach(function (id) {
        var doc = docFor(id);
        if (!doc) return;
        var adapter = adapters[id];
        var sem = state.semantic[id];
        // Reconcile the COMPLETE message list from state on every paint. There
        // is deliberately no once-cache: a cache outside reduce() is exactly
        // what made seek(t) differ from play-through-to(t).
        if (adapter && adapter.reconcileMessages) {
          adapter.reconcileMessages(doc, sem.messages || []);
        }
        if (adapter && adapter.apply) adapter.apply(doc, sem);
      });

      // chapter label
      var chapters = story.chapters || [];
      var chapter = chapters[0];
      for (var i = 0; i < chapters.length; i++) {
        if (t >= (chapters[i].at || 0)) chapter = chapters[i];
      }
      if (labelEl && chapter) labelEl.textContent = chapter.label;
      renderRail(chapter);
    }

    function renderRail(active) {
      if (!rail) return;
      var chapters = story.chapters || [];
      rail.innerHTML = chapters
        .map(function (ch, i) {
          var isActive = active && ch.id === active.id;
          var isDone = active && (ch.at || 0) < (active.at || 0);
          return (
            '<button type="button" class="story-rail__step' +
            (isActive ? " is-active" : "") +
            (isDone ? " is-done" : "") +
            '" data-goto-time="' +
            (ch.at || 0) +
            '"><span class="story-rail__num">' +
            (i + 1) +
            '</span><span class="story-rail__title">' +
            ch.label +
            "</span></button>"
          );
        })
        .join("");
      rail.querySelectorAll("[data-goto-time]").forEach(function (btn) {
        btn.addEventListener("click", function () {
          pause();
          seek(Number(btn.getAttribute("data-goto-time")));
        });
      });
    }

    function paint() {
      var state = reduce(story, t);
      applyState(state);
      return state;
    }

    function frame(now) {
      if (!playing) return;
      if (lastFrame == null) lastFrame = now;
      var dt = now - lastFrame;
      lastFrame = now;
      if (!document.hidden) t += dt;
      if (t >= duration) {
        if (story.loop !== false) t = 0;
        else {
          t = duration;
          pause();
        }
      }
      paint();
      raf = requestAnimationFrame(frame);
    }

    function play() {
      if (playing) return;
      playing = true;
      lastFrame = null;
      if (playBtn) {
        playBtn.setAttribute("aria-pressed", "true");
        playBtn.textContent = "Pause";
      }
      raf = requestAnimationFrame(frame);
    }

    function pause() {
      playing = false;
      if (raf) cancelAnimationFrame(raf);
      raf = null;
      lastFrame = null;
      if (playBtn) {
        playBtn.setAttribute("aria-pressed", "false");
        playBtn.textContent = "Play";
      }
    }

    function seek(ms) {
      // No cache to reset: every paint reconciles from reduce(story, t) alone.
      t = clamp(ms, 0, duration);
      paint();
    }

    function restart() {
      seek(0);
      play();
    }

    function getSemanticDigest() {
      var state = reduce(story, t);
      var dig = { t: Math.round(t), visible: [], sem: {} };
      Object.keys(state.visible).forEach(function (id) {
        if (state.visible[id]) dig.visible.push(id);
      });
      Object.keys(state.semantic).forEach(function (id) {
        var s = state.semantic[id];
        dig.sem[id] = {
          search: s.search || s.filter || "",
          composer: s.composer || "",
          text: s.text ? String(s.text).length : 0,
          selectedIndex: s.selectedIndex,
          streamPhase: s.streamPhase,
          lines: s.lines ? s.lines.length : undefined,
          terminal: s.lines
            ? s.lines
                .map(function (l) {
                  return typeof l === "string" ? l : l.text || "";
                })
                .join("\\n")
                .slice(0, 80)
            : undefined,
        };
      });
      return dig;
    }

    if (playBtn) {
      playBtn.addEventListener("click", function () {
        if (playing) pause();
        else play();
      });
    }
    var nextBtn = root.querySelector("[data-story-next]");
    var prevBtn = root.querySelector("[data-story-prev]");
    if (nextBtn) {
      nextBtn.addEventListener("click", function () {
        pause();
        var chapters = story.chapters || [];
        var next = chapters.find(function (ch) {
          return (ch.at || 0) > t + 1;
        });
        seek(next ? next.at : duration);
      });
    }
    if (prevBtn) {
      prevBtn.addEventListener("click", function () {
        pause();
        var chapters = (story.chapters || []).slice().reverse();
        var prev = chapters.find(function (ch) {
          return (ch.at || 0) < t - 1;
        });
        seek(prev ? prev.at : 0);
      });
    }

    var api = {
      play: play,
      pause: pause,
      restart: restart,
      seek: seek,
      getTime: function () {
        return t;
      },
      getDuration: function () {
        return duration;
      },
      getState: function () {
        return reduce(story, t);
      },
      getSemanticDigest: getSemanticDigest,
      story: story,
    };
    global.__SK_STORY__ = api;

    waitReady().then(function () {
      paint();
      if (autoplay) play();
    });

    return api;
  }

  global.StoryPlayer = {
    defineStory: function (def) {
      return def;
    },
    mount: mount,
    reduce: reduce,
    typePrefix: typePrefix,
  };
})(typeof window !== "undefined" ? window : globalThis);

/**
 * Surface adapters: mutate canonical screen fixture DOM inside story iframes.
 * Typing places the caret AFTER the text (blinks); filters hide/show real rows.
 */
(function (global) {
  "use strict";

  function q(doc, sel) {
    return doc.querySelector(sel);
  }
  function qa(doc, sel) {
    return Array.from(doc.querySelectorAll(sel));
  }

  /** Link the adapter stylesheet into the iframe document.
   *  Visual values live in story-adapter.css so the mockup honesty lint can
   *  see them; injecting a <style> string here was a route around the lint. */
  function ensureEmbedStyles(doc) {
    if (doc.getElementById("sk-story-adapter-style")) return;
    var link = doc.createElement("link");
    link.id = "sk-story-adapter-style";
    link.rel = "stylesheet";
    link.href = "../../stories/shared/story-adapter.css";
    (doc.head || doc.documentElement).appendChild(link);
  }

  function findShell(doc) {
    return q(
      doc,
      ".sk-search-shell, .sk-actions-search, .sk-arg-input, .sk-chat-input, .sk-agent-chat-composer__body",
    );
  }

  function textClassFor(shell) {
    if (shell.classList.contains("sk-agent-chat-composer__body")) return "sk-agent-chat-composer__text";
    if (shell.classList.contains("sk-chat-input")) return "sk-chat-input-text";
    if (shell.classList.contains("sk-arg-input")) return "sk-arg-text";
    return "sk-search-text";
  }

  function caretSelector() {
    return ".sk-caret, .sk-arg-caret, .sk-agent-chat-composer__caret";
  }

  /**
   * Paint typed text with caret at the end (real typing feel).
   */
  function setSearch(doc, text) {
    ensureEmbedStyles(doc);
    var shell = findShell(doc);
    if (!shell) return;
    var placeholder = shell.querySelector(
      ".sk-placeholder, .sk-arg-placeholder, .sk-chat-placeholder, .sk-agent-chat-composer__placeholder",
    );
    var caret = shell.querySelector(caretSelector());
    var existing = shell.querySelector(
      ".sk-search-text, .sk-arg-text, .sk-chat-input-text, .sk-agent-chat-composer__text",
    );

    if (text == null || text === "") {
      if (existing) existing.remove();
      if (placeholder) {
        placeholder.hidden = false;
        placeholder.style.display = "";
      }
      if (caret) {
        caret.classList.remove("is-typing");
        caret.style.left = "";
        caret.style.position = "";
      }
      return;
    }

    if (placeholder) {
      placeholder.hidden = true;
      placeholder.style.display = "none";
    }
    if (!existing) {
      existing = doc.createElement("span");
      existing.className = textClassFor(shell);
      // text first, caret after (in flow for measure); caret may be absolute
      if (caret) shell.insertBefore(existing, caret);
      else shell.appendChild(existing);
    }
    existing.textContent = text;

    if (caret) {
      caret.classList.add("is-typing");
      // Absolute caret after measured text (matches GPUI overlay caret)
      var shellRect = shell.getBoundingClientRect();
      var textRect = existing.getBoundingClientRect();
      var left = textRect.right - shellRect.left;
      // fallback if not laid out yet
      if (!textRect.width && text.length) {
        left =
          (parseFloat(getComputedStyle(shell).paddingLeft) || 0) +
          text.length * (parseFloat(getComputedStyle(shell).fontSize) || 14) * 0.55;
      }
      caret.style.position = "absolute";
      caret.style.left = Math.max(0, left) + "px";
      caret.style.top = "";
      if (!caret.style.top) {
        // keep vertical centering from screen CSS when possible
      }
    }
  }

  function rowSearchBlob(row) {
    var name = row.querySelector(".sk-list-row__name");
    var desc = row.querySelector(".sk-list-row__description");
    return (
      ((name && name.textContent) || "") +
      " " +
      ((desc && desc.textContent) || "")
    )
      .trim()
      .toLowerCase();
  }

  /**
   * Filter list rows against query (name + description). Updates section label.
   */
  function filterAndSelectRows(doc, opts) {
    ensureEmbedStyles(doc);
    var rows = qa(doc, ".sk-list .sk-list-row, .sk-clipboard-rows .sk-list-row, .sk-list-row");
    // de-dupe
    rows = rows.filter(function (r, i, arr) {
      return arr.indexOf(r) === i;
    });
    var query = (opts.query || "").toLowerCase().trim();
    var selectedIndex = opts.selectedIndex;
    var visible = [];

    rows.forEach(function (row) {
      var blob = rowSearchBlob(row);
      var match = !query || blob.indexOf(query) !== -1;
      if (match) {
        row.removeAttribute("data-story-hidden");
        row.hidden = false;
        row.style.display = "";
        visible.push(row);
      } else {
        row.setAttribute("data-story-hidden", "true");
        row.hidden = true;
        row.style.display = "none";
      }
      row.removeAttribute("data-state");
    });

    // Section header: Flows → Results (N) when filtering (preserve icon)
    var headers = qa(doc, ".sk-section-header");
    headers.forEach(function (h) {
      if (!h.getAttribute("data-story-label")) {
        var tmp = h.cloneNode(true);
        var strip = tmp.querySelector(".sk-section-icon");
        if (strip) strip.remove();
        h.setAttribute("data-story-label", tmp.textContent.trim() || "Flows");
      }
      var icon = h.querySelector(".sk-section-icon");
      var label = query
        ? "Results (" + visible.length + ")"
        : h.getAttribute("data-story-label");
      h.textContent = "";
      if (icon) h.appendChild(icon);
      h.appendChild(doc.createTextNode(" " + label));
      if (query) h.setAttribute("data-story-results", "true");
      else h.removeAttribute("data-story-results");
    });

    if (selectedIndex == null) selectedIndex = 0;
    if (selectedIndex < 0) selectedIndex = 0;
    if (selectedIndex >= visible.length) selectedIndex = Math.max(0, visible.length - 1);
    if (visible[selectedIndex]) {
      visible[selectedIndex].setAttribute("data-state", "selected");
      try {
        visible[selectedIndex].scrollIntoView({ block: "nearest" });
      } catch (_) {}
    }
    return { visibleCount: visible.length, selectedIndex: selectedIndex, names: visible.map(function (r) {
      var n = r.querySelector(".sk-list-row__name");
      return n ? n.textContent.trim() : "";
    }) };
  }

  function ensureRows(doc, names) {
    // names: [{name, desc?}]
    if (!names || !names.length) return;
    var list = q(doc, ".sk-list");
    if (!list) return;
    var existing = qa(doc, ".sk-list .sk-list-row");
    var template = existing[0];
    if (!template) return;
    var have = {};
    existing.forEach(function (row) {
      var n = row.querySelector(".sk-list-row__name");
      if (n) have[n.textContent.trim().toLowerCase()] = true;
    });
    names.forEach(function (item) {
      var name = typeof item === "string" ? item : item.name;
      var desc = typeof item === "string" ? "" : item.desc || "";
      if (have[name.toLowerCase()]) return;
      var clone = template.cloneNode(true);
      clone.removeAttribute("data-state");
      clone.removeAttribute("data-story-hidden");
      clone.hidden = false;
      clone.style.display = "";
      var nameEl = clone.querySelector(".sk-list-row__name");
      if (nameEl) nameEl.textContent = name;
      var descEl = clone.querySelector(".sk-list-row__description");
      if (desc) {
        if (!descEl) {
          var copy = clone.querySelector(".sk-list-row__copy");
          if (copy) {
            descEl = doc.createElement("span");
            descEl.className = "sk-list-row__description";
            copy.appendChild(descEl);
          }
        }
        if (descEl) descEl.textContent = desc;
      } else if (descEl) {
        descEl.remove();
      }
      list.appendChild(clone);
      have[name.toLowerCase()] = true;
    });
  }

  function setFooter(doc, state) {
    var rail = q(doc, ".sk-footer-rail");
    if (!rail) return;
    var actions = qa(rail, ".sk-footer-action");
    if (!actions.length) return;
    function fill(btn, label, keys) {
      if (!btn) return;
      var lab = btn.querySelector(".sk-footer-label");
      if (lab && label != null) lab.textContent = label;
      var keycaps = qa(btn, ".sk-keycap");
      if (keys && keys.length) {
        if (keycaps.length !== keys.length) {
          keycaps.forEach(function (k) {
            k.remove();
          });
          keys.forEach(function (k) {
            var el = doc.createElement("kbd");
            el.className = "sk-keycap";
            el.textContent = k;
            btn.appendChild(el);
          });
        } else {
          keys.forEach(function (k, i) {
            keycaps[i].textContent = k;
          });
        }
      }
    }
    var run = rail.querySelector(".sk-footer-action--run") || actions[0];
    var act = rail.querySelector(".sk-footer-action--actions") || actions[1];
    var agent = rail.querySelector(".sk-footer-action--agent") || actions[2];
    if (state.runLabel != null) fill(run, state.runLabel, state.runKeys);
    if (state.actionsLabel != null) fill(act, state.actionsLabel, state.actionsKeys);
    if (state.hideAgent && agent) agent.hidden = true;
    else if (agent) {
      agent.hidden = false;
      if (state.agentLabel != null) fill(agent, state.agentLabel, state.agentKeys);
    }
    actions.forEach(function (btn) {
      btn.removeAttribute("data-selected");
    });
    if (state.selected === "run" && run) run.setAttribute("data-selected", "true");
    if (state.selected === "actions" && act) act.setAttribute("data-selected", "true");
    if (state.selected === "agent" && agent) agent.setAttribute("data-selected", "true");
  }

  function setNotesEditor(doc, text) {
    ensureEmbedStyles(doc);
    var pre = q(doc, ".sk-notes-md, .sk-notes-editor pre, .sk-notes-editor");
    if (!pre) return;
    var caret = pre.querySelector(".sk-caret, .story-caret-blink");
    if (!caret) {
      caret = doc.createElement("span");
      caret.className = "sk-caret is-typing";
      caret.classList.add("sk-story-caret");
    } else {
      caret.classList.add("is-typing");
    }
    // text + trailing caret
    while (pre.firstChild) pre.removeChild(pre.firstChild);
    pre.appendChild(doc.createTextNode(text || ""));
    pre.appendChild(caret);
  }

  function setDayLines(doc, lines) {
    ensureEmbedStyles(doc);
    var editor = q(doc, ".sk-day-page-editor, .sk-day-page");
    if (!editor) return;
    var existing = qa(editor, ".sk-day-page-line");
    var template = existing[0];
    while (existing.length < lines.length && template) {
      var clone = template.cloneNode(true);
      editor.appendChild(clone);
      existing.push(clone);
    }
    // remove old caret
    qa(editor, ".sk-day-caret, .sk-caret.is-typing").forEach(function (c) {
      if (!c.classList.contains("sk-day-page-line")) c.remove();
    });
    existing.forEach(function (el, i) {
      if (i < lines.length) {
        el.hidden = false;
        el.style.display = "";
        el.textContent = lines[i];
      } else {
        el.hidden = true;
        el.style.display = "none";
      }
    });
    // caret on last visible line
    if (lines.length && existing[lines.length - 1]) {
      var caret = doc.createElement("span");
      caret.className = "sk-caret is-typing sk-day-caret";
      caret.classList.add("sk-story-caret", "sk-story-caret--input");
      existing[lines.length - 1].appendChild(caret);
    }
  }

  function setTerminalLines(doc, lines) {
    ensureEmbedStyles(doc);
    var entity = q(doc, ".sk-term-entity, .sk-term-content");
    if (!entity) {
      var content = q(doc, ".sk-window__content");
      if (content) entity = content.querySelector(".sk-term-entity") || content;
    }
    if (!entity) return;
    var rows = qa(entity, ".sk-term-row");
    if (!rows.length) {
      var pre = entity.querySelector("pre") || entity;
      pre.textContent = lines
        .map(function (l) {
          return typeof l === "string" ? l : l.text || "";
        })
        .join("\n");
      return;
    }
    var template = rows[0];
    while (rows.length < lines.length) {
      var c = template.cloneNode(true);
      entity.appendChild(c);
      rows.push(c);
    }
    rows.forEach(function (row, i) {
      if (i >= lines.length) {
        row.hidden = true;
        return;
      }
      row.hidden = false;
      var line = lines[i];
      if (typeof line === "string") {
        row.textContent = line;
      } else if (line.html) {
        row.innerHTML = line.html;
      } else {
        row.textContent = line.text || "";
        if (line.cursor) {
          var cur = doc.createElement("span");
          cur.className = "sk-term-cursor is-typing";
          row.appendChild(cur);
        }
      }
    });
  }

  function setChatComposer(doc, text) {
    setSearch(doc, text);
  }

  /** Reconcile the COMPLETE ordered message list into the transcript.
   *
   *  Replaces the previous append-with-random-id model, which was the second
   *  half of the seek-determinism defect: random ids made DOM unaddressable,
   *  and appending meant seeking backward could not remove a message. Now the
   *  DOM is a pure function of `messages`, so any seek order converges.
   *
   *  Non-message nodes in the transcript (fixture content, receipts) are left
   *  untouched — only [data-story-msg] nodes are owned by the story.
   */
  function reconcileChatMessages(doc, messages) {
    var host = q(doc, ".sk-chat-messages, .sk-agent-chat-transcript, .sk-conversation__transcript");
    if (!host) return;
    var isAgentLike = !host.classList.contains("sk-chat-messages");
    var wanted = messages || [];
    var wantedIds = wanted.map(function (m) { return m.id; });

    // Remove story-owned nodes that are no longer in state (backward seek).
    qa(doc, "[data-story-msg]").forEach(function (el) {
      if (wantedIds.indexOf(el.getAttribute("data-story-msg")) === -1) {
        if (el.parentNode) el.parentNode.removeChild(el);
      }
    });

    wanted.forEach(function (msg) {
      var el = host.querySelector('[data-story-msg="' + msg.id + '"]');
      if (!el) {
        el = doc.createElement("div");
        el.setAttribute("data-story-msg", msg.id);
        host.appendChild(el);
      }
      el.setAttribute("data-turn-state", msg.state || "complete");
      if (isAgentLike) {
        el.className =
          msg.role === "user"
            ? "sk-agent-chat-turn sk-agent-chat-turn--user"
            : "sk-agent-chat-turn sk-agent-chat-turn--assistant";
        el.textContent = msg.text || "";
      } else {
        el.className = "sk-chat-turn";
        el.innerHTML =
          '<div class="sk-chat-card"><div class="' +
          (msg.role === "user" ? "sk-chat-user" : "sk-chat-md") +
          '">' +
          (msg.role === "user"
            ? msg.text || ""
            : '<p class="sk-chat-md__p">' + (msg.text || "") + "</p>") +
          "</div></div>";
      }
    });

    var last = host.querySelector("[data-story-msg]:last-child");
    if (last) {
      try {
        last.scrollIntoView({ block: "nearest" });
      } catch (_) {}
    }
  }

  function appendChatMessage(doc, role, text) {
    var messages = q(doc, ".sk-chat-messages, .sk-agent-chat-transcript");
    if (!messages) return;
    // Deterministic id derived from content — no Math.random.
    var id = "story-msg-" + role + "-" + String(text || "").length;
    var el = doc.createElement("div");
    el.setAttribute("data-story-msg", id);
    if (messages.classList.contains("sk-agent-chat-transcript")) {
      el.className =
        role === "user"
          ? "sk-agent-chat-turn sk-agent-chat-turn--user"
          : "sk-agent-chat-turn sk-agent-chat-turn--assistant";
      el.textContent = text;
    } else {
      el.className = "sk-chat-turn";
      el.innerHTML =
        '<div class="sk-chat-card"><div class="' +
        (role === "user" ? "sk-chat-user" : "sk-chat-md") +
        '">' +
        (role === "user" ? text : '<p class="sk-chat-md__p">' + text + "</p>") +
        "</div></div>";
    }
    messages.appendChild(el);
    try {
      el.scrollIntoView({ block: "nearest" });
    } catch (_) {}
    return id;
  }

  function updateStoryMsg(doc, msgId, text) {
    var el = q(doc, '[data-story-msg="' + msgId + '"]');
    if (!el) return;
    var p = el.querySelector(".sk-chat-md__p");
    if (p) p.textContent = text;
    else el.textContent = text;
  }

  function applyMain(doc, state) {
    if (state.ensureRows) ensureRows(doc, state.ensureRows);
    if (state.search != null) setSearch(doc, state.search);
    var qtext =
      state.filter != null ? state.filter : state.search != null ? state.search : null;
    if (qtext != null || state.selectedIndex != null) {
      filterAndSelectRows(doc, {
        query: qtext || "",
        selectedIndex: state.selectedIndex != null ? state.selectedIndex : 0,
      });
    }
    if (state.footer) setFooter(doc, state.footer);
  }

  var adapters = {
    "main-menu": {
      apply: applyMain,
      inspect: function (doc) {
        var sel = q(doc, '.sk-list-row[data-state="selected"] .sk-list-row__name');
        var search = q(doc, ".sk-search-text");
        var visible = qa(doc, ".sk-list-row").filter(function (r) {
          return r.getAttribute("data-story-hidden") !== "true" && !r.hidden;
        });
        return {
          search: search ? search.textContent : "",
          selected: sel ? sel.textContent.trim() : "",
          visibleCount: visible.length,
        };
      },
    },
    "actions-dialog": {
      apply: function (doc, state) {
        if (state.search != null) setSearch(doc, state.search);
        filterAndSelectRows(doc, {
          query: state.filter != null ? state.filter : state.search || "",
          selectedIndex: state.selectedIndex || 0,
        });
      },
      inspect: function (doc) {
        var sel = q(doc, '.sk-list-row[data-state="selected"] .sk-list-row__name');
        return { selected: sel ? sel.textContent.trim() : "" };
      },
    },
    "arg-prompt": {
      apply: function (doc, state) {
        if (state.input != null) setSearch(doc, state.input);
        filterAndSelectRows(doc, {
          query: "",
          selectedIndex: state.selectedIndex || 0,
        });
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var sel = q(doc, '.sk-list-row[data-state="selected"] .sk-list-row__name');
        return { selected: sel ? sel.textContent.trim() : "" };
      },
    },
    "confirm-popup": {
      apply: function (doc, state) {
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function () {
        return { surface: "confirm" };
      },
    },
    "clipboard-history": {
      apply: function (doc, state) {
        if (state.search != null) setSearch(doc, state.search);
        filterAndSelectRows(doc, {
          query: state.filter != null ? state.filter : state.search || "",
          selectedIndex: state.selectedIndex || 0,
        });
        if (state.preview != null) {
          var content = q(doc, ".sk-clipboard-preview__content");
          if (content) content.textContent = state.preview;
        }
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var sel = q(doc, '.sk-list-row[data-state="selected"] .sk-list-row__name');
        return { selected: sel ? sel.textContent.trim() : "" };
      },
    },
    "day-page": {
      apply: function (doc, state) {
        if (state.lines) setDayLines(doc, state.lines);
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var lines = qa(doc, ".sk-day-page-line").filter(function (el) {
          return !el.hidden;
        });
        return {
          lineCount: lines.length,
          last: lines.length ? lines[lines.length - 1].textContent : "",
        };
      },
    },
    notes: {
      apply: function (doc, state) {
        if (state.text != null) setNotesEditor(doc, state.text);
        if (state.title != null) {
          var tb = q(doc, ".sk-notes-titlebar");
          if (tb) tb.textContent = state.title;
        }
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var pre = q(doc, ".sk-notes-md, .sk-notes-editor pre");
        var t = pre ? pre.textContent : "";
        return { length: t.length, prefix: t.slice(0, 40) };
      },
    },
    settings: {
      apply: function (doc, state) {
        if (state.search != null) setSearch(doc, state.search);
        filterAndSelectRows(doc, {
          query: state.filter != null ? state.filter : state.search || "",
          selectedIndex: state.selectedIndex || 0,
        });
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var sel = q(doc, '.sk-list-row[data-state="selected"] .sk-list-row__name');
        return { selected: sel ? sel.textContent.trim() : "" };
      },
    },
    "chat-prompt": {
      apply: function (doc, state) {
        if (state.composer != null) setChatComposer(doc, state.composer);
        if (state.streamText != null && state.streamMsgId) {
          updateStoryMsg(doc, state.streamMsgId, state.streamText);
        }
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var t = q(doc, ".sk-chat-input-text");
        return { composer: t ? t.textContent : "" };
      },
      appendMessage: appendChatMessage,
      reconcileMessages: reconcileChatMessages,
    },
    "terminal-prompt": {
      apply: function (doc, state) {
        if (state.lines) setTerminalLines(doc, state.lines);
      },
      inspect: function (doc) {
        return { rows: qa(doc, ".sk-term-row").length };
      },
    },
    "agent-chat": {
      apply: function (doc, state) {
        if (state.composer != null) setChatComposer(doc, state.composer);
        if (state.sendState) {
          var btn = q(doc, ".sk-agent-chat-send");
          if (btn) btn.setAttribute("data-state", state.sendState);
        }
        if (state.streamText != null && state.streamMsgId) {
          updateStoryMsg(doc, state.streamMsgId, state.streamText);
        }
        if (state.footer) setFooter(doc, state.footer);
      },
      inspect: function (doc) {
        var t = q(doc, ".sk-agent-chat-composer__text");
        return { composer: t ? t.textContent : "" };
      },
      appendMessage: appendChatMessage,
      reconcileMessages: reconcileChatMessages,
    },

    // Conversation shell (PROPOSAL fixture — screens/conversation/).
    //
    // The whole point of this adapter is that a MODE CHANGE MUST NOT CHANGE
    // GEOMETRY. It therefore only ever writes text and chips into elements that
    // already exist, and never adds, removes, hides, or resizes any of the five
    // same-shell nodes. If proving "one shell, three modes" required structural
    // edits here, the fixture would be wrong — not this adapter.
    conversation: {
      apply: function (doc, state) {
        ensureEmbedStyles(doc);
        var shell = q(doc, ".sk-conversation");
        if (!shell) return;

        if (state.mode) {
          shell.setAttribute("data-mode", state.mode);
          shell.setAttribute(
            "data-durability",
            state.mode === "ask" ? "ephemeral" : "durable",
          );
        }
        if (state.contextPolicy) {
          var lane = q(doc, "[data-conversation-context-lane]");
          if (lane) lane.setAttribute("data-context-policy", state.contextPolicy);
        }

        var id = state.identity || {};
        var modeLabel = q(doc, "[data-conversation-mode-label]");
        if (modeLabel && id.label) modeLabel.textContent = id.label;
        var subject = q(doc, "[data-conversation-subject]");
        if (subject) subject.textContent = id.subject || "";
        var authority = q(doc, "[data-conversation-authority]");
        if (authority && id.authority) authority.textContent = id.authority;

        // Context lane: policy item is structural and always present; grants
        // are appended AFTER it, never in place of it.
        var lane2 = q(doc, "[data-conversation-context-lane]");
        if (lane2 && state.grants) {
          qa(doc, "[data-story-grant]").forEach(function (el) {
            if (el.parentNode) el.parentNode.removeChild(el);
          });
          state.grants.forEach(function (g) {
            var chip = doc.createElement("span");
            chip.setAttribute("data-story-grant", g.id);
            chip.setAttribute("data-state", g.state || "staged");
            chip.className = "sk-conversation__grant";
            chip.textContent = g.label + (g.detail ? " · " + g.detail : "");
            lane2.appendChild(chip);
          });
        }

        var policy = q(doc, ".sk-conversation__policy");
        if (policy && state.policyLabel) policy.textContent = state.policyLabel;

        var placeholder = q(doc, ".sk-conversation__placeholder");
        if (placeholder && state.placeholder) placeholder.textContent = state.placeholder;

        // Footer slot 3 is mode-specific: Ask->Work, Work->Context,
        // Flow->Definition. It is deliberately NOT a universal "Agent" label.
        if (state.footerModeLabel) {
          var fm = q(doc, "[data-footer-mode-label]");
          if (fm) fm.textContent = state.footerModeLabel;
        }
        if (state.footerPrimaryLabel) {
          var fp = q(doc, "[data-footer-primary-label]");
          if (fp) fp.textContent = state.footerPrimaryLabel;
        }

        if (state.receipt) {
          var r = q(doc, "[data-conversation-receipt]");
          if (r) {
            r.setAttribute("data-outcome", state.receipt.outcome || "none");
            var a = (state.receipt.attempted || []).length;
            var res = (state.receipt.resolved || []).length;
            var f = (state.receipt.failed || []).length;
            r.textContent = "Context: " + a + " attempted · " + res + " resolved · " + f + " failed";
          }
        }
        if (state.composer != null) setSearch(doc, state.composer);
      },
      reconcileMessages: reconcileChatMessages,
      inspect: function (doc) {
        var shell = q(doc, ".sk-conversation");
        return {
          mode: shell ? shell.getAttribute("data-mode") : null,
          grants: qa(doc, "[data-story-grant]").length,
        };
      },
    },
  };

  global.StorySurfaces = {
    adapters: adapters,
    setSearch: setSearch,
    filterAndSelectRows: filterAndSelectRows,
    setFooter: setFooter,
    setNotesEditor: setNotesEditor,
    setDayLines: setDayLines,
    setTerminalLines: setTerminalLines,
    ensureRows: ensureRows,
    appendChatMessage: appendChatMessage,
  };
})(typeof window !== "undefined" ? window : globalThis);

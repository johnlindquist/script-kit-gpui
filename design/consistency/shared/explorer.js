const STORAGE_KEY = "script-kit.consistency.review.v1";

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

function escapeHtml(value = "") {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function loadReview() {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return value && typeof value === "object" ? value : {};
  } catch {
    return {};
  }
}

function saveReview(review) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(review));
}

function decisionFor(review, groupId, taskId) {
  return review.groups?.[groupId]?.tasks?.[taskId] || { verdict: null, note: "", updatedAt: null };
}

function writeDecision(review, groupId, taskId, patch) {
  review.schemaVersion = 1;
  review.groups ||= {};
  review.groups[groupId] ||= { tasks: {} };
  review.groups[groupId].tasks ||= {};
  review.groups[groupId].tasks[taskId] = {
    ...decisionFor(review, groupId, taskId),
    ...patch,
    updatedAt: new Date().toISOString(),
  };
  saveReview(review);
}

function announce(message) {
  const live = $("[data-live-region]");
  if (live) live.textContent = message;
}

async function loadManifest() {
  const app = $("#consistency-app");
  const src = app?.dataset.manifest;
  if (!src) throw new Error("Missing data-manifest on #consistency-app");
  const response = await fetch(src, { cache: "no-store" });
  if (!response.ok) throw new Error(`Could not load ${src}: ${response.status}`);
  return response.json();
}

function reviewCounts(manifest, review) {
  const counts = { approve: 0, revise: 0, reject: 0, decided: 0, total: manifest.taskCount };
  for (const group of manifest.groups) {
    for (const task of group.taskRecords) {
      const verdict = decisionFor(review, group.id, task.id).verdict;
      if (verdict && verdict in counts) {
        counts[verdict] += 1;
        counts.decided += 1;
      }
    }
  }
  return counts;
}

function exportDecisions(manifest, review) {
  const receipt = {
    schemaVersion: 1,
    exportedAt: new Date().toISOString(),
    manifest: { groupCount: manifest.groupCount, taskCount: manifest.taskCount },
    review,
  };
  const blob = new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "script-kit-consistency-decisions.json";
  link.click();
  URL.revokeObjectURL(url);
  announce("Exported consistency decisions.");
}

function bindGlobalActions(manifest, review) {
  $$('[data-export-decisions]').forEach((button) => {
    button.addEventListener("click", () => exportDecisions(manifest, review));
  });
  $$('[data-reset-decisions]').forEach((button) => {
    button.addEventListener("click", () => {
      if (!window.confirm("Reset every local consistency verdict and note?")) return;
      localStorage.removeItem(STORAGE_KEY);
      window.location.reload();
    });
  });
}

function progressMarkup(counts) {
  const percent = counts.total ? Math.round((counts.decided / counts.total) * 100) : 0;
  return `
    <div class="cx-progress-line"><strong>${counts.decided} of ${counts.total}</strong><span>decisions recorded</span></div>
    <div class="cx-progress-track" aria-label="${percent} percent reviewed"><span style="--cx-progress:${percent}%"></span></div>
    <div class="cx-progress-line"><span>${counts.approve} approve</span><span>${counts.revise} revise</span><span>${counts.reject} reject</span></div>
  `;
}

function renderIndex(manifest, review) {
  const counts = reviewCounts(manifest, review);
  $("[data-review-summary]").innerHTML = progressMarkup(counts);
  $("[data-group-grid]").innerHTML = manifest.groups.map((group, index) => {
    const decided = group.taskRecords.filter((task) => decisionFor(review, group.id, task.id).verdict).length;
    const reviewed = decided === group.taskRecords.length;
    return `
      <a class="cx-group-card" href="./groups/${group.id}.html" data-group-card="${escapeHtml(group.id)}">
        <div class="cx-group-card__index"><span>${String(index + 1).padStart(2, "0")}</span><span>${group.taskRecords.length} fixes</span></div>
        <div>
          <h2>${escapeHtml(group.title)}</h2>
          <p>${escapeHtml(group.question)}</p>
        </div>
        <div class="cx-group-card__footer">
          <span class="cx-group-card__progress">${reviewed ? "Review complete" : `${decided}/${group.taskRecords.length} decided`}</span>
          <span class="cx-group-card__arrow" aria-hidden="true">→</span>
        </div>
      </a>`;
  }).join("");
}

function statusLabel(task) {
  const status = task.status.match(/`([^`]+)`/)?.[1] || task.status;
  return status.length > 58 ? `${status.slice(0, 55)}…` : status;
}

function renderGroup(manifest, group, review) {
  const decided = group.taskRecords.filter((task) => decisionFor(review, group.id, task.id).verdict).length;
  const referenceHref = group.fixture.replace("../mockups", "../../mockups");
  const header = $("[data-group-header]");
  header.innerHTML = `
    <div>
      <p class="cx-kicker">Group ${manifest.groups.findIndex((item) => item.id === group.id) + 1} of ${manifest.groupCount}</p>
      <h1>${escapeHtml(group.title)}</h1>
      <p class="cx-group-hero__question">${escapeHtml(group.question)}</p>
    </div>
    <div class="cx-group-hero__meta">
      <span><span>Coverage</span><strong>${group.taskRecords.length} tasks</strong></span>
      <span><span>Review</span><strong data-group-progress>${decided}/${group.taskRecords.length}</strong></span>
      <span><span>Baseline</span><a class="cx-reference-link" href="${escapeHtml(referenceHref)}">Open current fixture ↗</a></span>
      <span><span>Product edits</span><strong>None</strong></span>
    </div>`;
  header.setAttribute("aria-busy", "false");

  $("[data-task-nav]").innerHTML = group.taskRecords.map((task) => {
    const reviewed = Boolean(decisionFor(review, group.id, task.id).verdict);
    return `<a href="#task-${escapeHtml(task.id)}" data-nav-task="${escapeHtml(task.id)}" data-reviewed="${reviewed}">${escapeHtml(task.id)}</a>`;
  }).join("");

  $("[data-scene-list]").innerHTML = group.taskRecords.map((task, index) => renderScene(group, task, index, review)).join("");
  bindDecisions(group, review);
  bindCompareControls();
  bindTaskFilter();
}

function renderScene(group, task, index, review) {
  const decision = decisionFor(review, group.id, task.id);
  return `
    <article class="cx-scene" id="task-${escapeHtml(task.id)}" data-scene-id="${escapeHtml(task.id)}" data-search="${escapeHtml(`${task.id} ${task.title} ${task.owners}`.toLowerCase())}">
      <header class="cx-scene__header">
        <div>
          <div class="cx-scene__identity">
            <span class="cx-task-id">${escapeHtml(task.id)}</span>
            <span class="cx-status">${escapeHtml(statusLabel(task))}</span>
          </div>
          <h2>${escapeHtml(task.title)}</h2>
        </div>
        <span class="cx-scene__number">${String(index + 1).padStart(2, "0")} / ${String(group.taskRecords.length).padStart(2, "0")}</span>
      </header>

      <div class="cx-compare" data-task-compare="${escapeHtml(task.id)}">
        <section class="cx-panel" data-side="before" aria-labelledby="${escapeHtml(task.id)}-before-title">
          <header class="cx-panel__caption"><strong id="${escapeHtml(task.id)}-before-title">Before</strong><span class="cx-truth-label">CURRENT · SOURCE-DERIVED</span></header>
          <div class="cx-product-viewport">
            <div class="cx-product-frame" data-product-emulation data-phase="before" data-group-visual="${escapeHtml(group.id)}">
              ${renderProduct(group.id, task, "before")}
            </div>
          </div>
        </section>
        <section class="cx-panel" data-side="after" aria-labelledby="${escapeHtml(task.id)}-after-title">
          <header class="cx-panel__caption"><strong id="${escapeHtml(task.id)}-after-title">After</strong><span class="cx-truth-label">PROPOSAL · NOT IMPLEMENTED</span></header>
          <div class="cx-product-viewport">
            <div class="cx-product-frame" data-product-emulation data-phase="after" data-group-visual="${escapeHtml(group.id)}">
              ${renderProduct(group.id, task, "after")}
            </div>
          </div>
        </section>
      </div>

      <div class="cx-scene__details">
        <aside class="cx-evidence" aria-label="Source and proof context" data-annotation-outside-frame="true">
          <dl><dt>${task.id === "WF-014" ? "Decision conflict" : "Current surprise"}</dt><dd>${escapeHtml(task.before)}</dd></dl>
          <dl><dt>Proposal</dt><dd>${escapeHtml(task.after)}</dd></dl>
          <dl><dt>Owner</dt><dd><code>${escapeHtml(task.owners)}</code></dd></dl>
          <dl><dt>Acceptance</dt><dd>${escapeHtml(task.acceptance)}</dd></dl>
          <dl><dt>Proof target</dt><dd>${escapeHtml(task.proof)}</dd></dl>
          <dl><dt>Guardrail</dt><dd>${escapeHtml(task.guardrail)}</dd></dl>
        </aside>

        <form class="cx-decision" data-decision-form data-task-id="${escapeHtml(task.id)}">
          <fieldset>
            <legend>Record a verdict for ${escapeHtml(task.id)}</legend>
            ${["approve", "revise", "reject"].map((verdict) => `
              <label class="cx-verdict-option">
                <input type="radio" name="verdict-${escapeHtml(task.id)}" value="${verdict}" ${decision.verdict === verdict ? "checked" : ""}>
                <span>${verdict[0].toUpperCase() + verdict.slice(1)}</span>
              </label>`).join("")}
          </fieldset>
          <p class="cx-recommendation"><strong>Recommendation:</strong> ${escapeHtml(task.recommendation)}</p>
          <label for="note-${escapeHtml(task.id)}">Review note</label>
          <textarea id="note-${escapeHtml(task.id)}" data-decision-note placeholder="What should change before approval?">${escapeHtml(decision.note || "")}</textarea>
          <button type="button" class="cx-clear-decision" data-clear-decision>Clear decision</button>
        </form>
      </div>
    </article>`;
}

function renderProduct(groupId, task, phase) {
  const renderers = {
    "proof-truth": renderProof,
    "cues-actions": renderCues,
    "context-identity": renderContext,
    "rows-sections": renderRows,
    "inputs-popups": renderInputs,
    "states-recovery": renderStates,
    "conversations-flow": renderConversation,
    "notes-today": renderNotes,
    "dictation": renderDictation,
    "geometry-settings": renderGeometry,
    "accessibility-semantics": renderAccessibility,
    "governance-contracts": renderGovernance,
  };
  return renderers[groupId](task, phase);
}

function footer(actions, leading = "") {
  return `<footer class="cx-product-footer"><span>${escapeHtml(leading)}</span><span class="cx-product-footer__actions">${actions.map(([label, keys]) => `<span class="cx-product-action"><span>${escapeHtml(label)}</span>${keys.map((key) => `<kbd class="sk-keycap">${escapeHtml(key)}</kbd>`).join("")}</span>`).join("")}</span></footer>`;
}

function row(name, description = "", selected = false, suffix = "") {
  return `<div class="sk-list-row" data-state="${selected ? "selected" : "rest"}"><div class="sk-list-row__surface"><span class="sk-list-row__copy"><span class="sk-list-row__name">${escapeHtml(name)}</span>${description ? `<span class="sk-list-row__description">${escapeHtml(description)}</span>` : ""}</span>${suffix}</div></div>`;
}

function renderProof(task, phase) {
  const isAfter = phase === "after";
  const proofState = task.id === "PF-011" ? (isAfter ? "INVALID_OBSERVER" : "PASS?") : (isAfter ? "EVALUABLE_PASS" : "INCOMPLETE");
  return `
    <div class="cx-product-header"><span class="cx-product-title">Proof receipt</span><span>${escapeHtml(task.id)}</span></div>
    <div class="cx-proof-stack">
      <div class="cx-proof-card" data-state="${isAfter ? "pass" : "blocked"}"><strong>${isAfter ? "Exact target" : "Surface found"}</strong>${isAfter ? "window · generation · commit" : "target identity not bound"}</div>
      <div class="cx-proof-card" data-state="${isAfter ? "pass" : "blocked"}"><strong>${isAfter ? "Required evidence" : "Available fields"}</strong>${isAfter ? "schema validated at producer" : "missing fields tolerated"}</div>
      <div class="cx-proof-card" data-state="${isAfter ? "pass" : "blocked"}"><strong>${proofState}</strong>${isAfter ? "classification earned" : "coverage mistaken for proof"}</div>
    </div>
    ${footer(isAfter ? [["Inspect", ["↵"]], ["Copy receipt", ["⌘", "C"]]] : [["Looks green", ["?"]]], isAfter ? "Redacted · comparable" : "Unknown observer")}`;
}

function renderCues(task, phase) {
  const after = phase === "after";
  const feedback = task.id === "UX-016";
  const staticHint = task.id === "UX-015";
  return `
    <div class="cx-product-header"><span class="cx-product-title">${feedback ? "Feedback controls" : staticHint ? "Hint semantics" : "Cue grammar"}</span><span>${escapeHtml(task.id)}</span></div>
    <div class="cx-cue-grid">
      <div class="cx-cue-row" data-selected="true"><span>${feedback ? "Copy completed" : staticHint ? "Filter results" : "Open Actions"}</span><span class="cx-cue-example">${feedback ? (after ? `<button class="cx-product-chip">Dismiss</button>` : `<span class="cx-cue-label">auto hides</span>`) : staticHint ? (after ? `<span class="cx-cue-label">Filter</span>` : `<button class="cx-product-chip">Filter</button>`) : `<kbd class="sk-keycap">⌘</kbd><kbd class="sk-keycap">K</kbd>`}</span></div>
      <div class="cx-cue-row"><span>${after ? "Attach context" : "Context"}</span><span class="cx-cue-example">${after ? `<span class="cx-cue-trigger">@</span><span class="cx-cue-label">trigger</span>` : `<kbd class="sk-keycap">@</kbd>`}</span></div>
      <div class="cx-cue-row"><span>${after ? "Capture syntax" : "Capture"}</span><span class="cx-cue-example">${after ? `<code class="cx-cue-syntax">;todo</code><span class="cx-cue-label">syntax</span>` : `<kbd class="sk-keycap">;</kbd><kbd class="sk-keycap">todo</kbd>`}</span></div>
    </div>
    ${footer(after ? [["Converse", ["↵"]], ["Actions", ["⌘", "K"]]] : [["Run", ["↵"]], ["Filter", ["F"]]])}`;
}

function renderContext(task, phase) {
  const after = phase === "after";
  return `
    <div class="cx-product-header"><span class="cx-product-title">Agent Chat</span><span>${after ? "Explicit roles" : "Similar pills"}</span></div>
    <div class="cx-product-chip-row">
      <span class="cx-product-chip" data-role="${after ? "identity" : "pill"}">${after ? "Agent · Claude Code" : "Claude Code"}</span>
      <span class="cx-product-chip" data-role="${after ? "context" : "pill"}">${after ? "From selection · README.md · next turn" : "README.md ×"}</span>
    </div>
    <div class="cx-product-body">
      <div class="cx-state-card">
        <span class="cx-state-marker">${after ? "DESTINATION" : "TARGET"}</span>
        <div class="cx-product-chip-row">
          ${(after ? ["App", "Today", "Quick AI", "Agent Chat"] : ["Paste", "Today", "Ask", "Send"]).map((label, index) => `<span class="cx-product-chip" data-role="${after ? "destination" : "pill"}" data-selected="${index === 3}">${label}</span>`).join("")}
        </div>
        <p>${after ? "Selection stages context. Ask and Send are explicit delivery verbs." : "The same silhouette can configure, remove, retarget, or submit."}</p>
      </div>
    </div>
    ${footer(after ? [["Send", ["↵"]], ["Remove context", ["⌫"]]] : [["Continue", ["↵"]]])}`;
}

function renderRows(task, phase) {
  const after = phase === "after";
  const marker = task.id === "UX-007" && after ? `<span class="cx-state-marker" aria-label="Selected">▌</span>` : "";
  const section = task.id === "UX-008" && after ? "RESULTS · 3" : "Results";
  return `
    <div class="cx-product-header"><span class="cx-product-title">${after ? "Shared state grammar" : "Local row formulas"}</span><span>${escapeHtml(task.id)}</span></div>
    <section class="sk-list">
      <div class="sk-section-slot" data-first="true"><div class="sk-section-header">${section}</div></div>
      ${row("AGY Info", after ? "Selected · activatable · stable ID" : "Selected by local palette", true, marker)}
      ${row("Loading files", after ? "Status · skipped by navigation" : "Not a header")}
      ${row("Show Info", after ? "Focused explanation · not activatable" : "Unavailable", false, after ? `<span class="cx-cue-label">Unavailable</span>` : "")}
    </section>
    ${footer(after ? [["Open", ["↵"]], ["Actions", ["⌘", "K"]]] : [["Run", ["↵"]]])}`;
}

function renderInputs(task, phase) {
  const after = phase === "after";
  const disabled = task.id === "UX-003";
  const lifecycle = task.id === "UX-014";
  return `
    <div class="cx-product-header"><span class="cx-product-title">${lifecycle ? "Popup stack" : "Actions"}</span><span>${after ? "Stable owner" : "Local owner"}</span></div>
    ${after ? `<div class="sk-search-shell"><span class="sk-caret"></span><span class="sk-placeholder">Search actions…</span></div>` : ""}
    <section class="sk-list">
      <div class="sk-section-slot" data-first="true"><div class="sk-section-header">${lifecycle ? "LAYERS" : "ACTIONS"}</div></div>
      ${row(lifecycle ? "Actions over References" : "Converse", disabled ? (after ? "Agent Chat needs setup" : "Appears executable") : after ? "Input state owns selection, paste, undo, and IME" : "Popup mutates a raw string", true, disabled && after ? `<span class="cx-cue-label">Unavailable</span>` : `<span class="sk-shortcut-cluster"><kbd class="sk-keycap">↵</kbd></span>`)}
      ${row(lifecycle ? "Escape" : "Show Info", lifecycle ? (after ? "Close one layer · restore prior focus" : "Consumer-specific cleanup") : "Open details")}
      ${row(lifecycle ? "External close" : "Copy Deep Link", lifecycle ? (after ? "Reconcile open=false · generation-safe" : "Stale open state possible") : "")}
    </section>
    ${!after ? `<div class="sk-search-shell"><span class="sk-caret"></span><span class="sk-placeholder">Search actions…</span></div>` : ""}
    ${footer(after ? [[disabled ? "Unavailable" : "Open", ["↵"]], ["Close", ["Esc"]]] : [["Select", ["↵"]]])}`;
}

function renderStates(task, phase) {
  const after = phase === "after";
  const rich = task.id === "GOV-001";
  return `
    <div class="cx-product-header"><span class="cx-product-title">${rich ? "About Script Kit" : "Semantic state"}</span><span>${escapeHtml(task.id)}</span></div>
    <div class="cx-state-card">
      ${after && !rich ? `<span class="cx-state-marker">${task.id === "UX-017" ? "! RECOVERY" : "GUIDANCE"}</span>` : ""}
      <h3>${rich ? "Created by John Lindquist" : "No results for “design system”"}</h3>
      <p>${rich ? (after ? "Rich composition remains on its capable owner: quick actions, updates, and acknowledgements." : "A compositional page can be mistaken for another generic empty-state owner.") : after ? "Try fewer words, clear the search, or ask Agent Chat. Tone is visible without a background wash." : "Tone exists in data, but Help and Recovery share the same visible anatomy."}</p>
      ${rich ? `<div class="cx-product-chip-row"><span class="cx-product-chip">GitHub</span><span class="cx-product-chip">Updates</span><span class="cx-product-chip">Acknowledgements</span></div>` : ""}
    </div>
    ${footer(rich ? [["Close", ["Esc"]]] : after ? [["Clear search", ["Esc"]], ["Ask Agent", ["⌘", "↵"]]] : [["Back", ["Esc"]]])}`;
}

function renderConversation(task, phase) {
  const after = phase === "after";
  const destructive = task.id === "SAFE-003";
  const flow = task.id === "WF-011";
  const cancellation = task.id === "WF-007";
  return `
    <div class="cx-chat">
      <div class="cx-product-header"><span class="cx-product-title">${flow ? "Flow · Deep Improvements" : "Conversation"}</span><span>${after ? "Claude · Project" : "codex · gpt"}</span></div>
      <div class="cx-chat-transcript">
        <div class="cx-chat-message cx-chat-message--user">Explain the current contract.</div>
        <div class="cx-chat-message cx-chat-message--assistant">${destructive ? (after ? "The previous 18 turns remain archived." : "New Conversation clears the stored snapshot.") : cancellation ? (after ? "Partial answer preserved. Stopped." : "Broken pipe") : flow ? (after ? "History · all 18 turns saved" : "History saved · 12 turns restored") : after ? "Commands appear only when this host can execute them." : "Run, Stop, Retry, and Close are host-local promises."}</div>
        <span class="cx-chat-status">${after ? (cancellation ? "Stopped · not an error" : "Ready") : "Host behavior varies"}</span>
      </div>
      ${footer(after ? destructive ? [["New Conversation", ["⌘", "L"]], ["Actions", ["⌘", "K"]]] : cancellation ? [["Retry", ["↵"]], ["Actions", ["⌘", "K"]]] : [["Send", ["↵"]], ["Actions", ["⌘", "K"]]] : [["Run", ["↵"]], ["Esc Desk", ["Esc"]]])}
    </div>`;
}

function renderNotes(task, phase) {
  const after = phase === "after";
  const geometry = task.id === "GEO-004" || task.id === "GEO-005";
  const browse = task.id === "WF-017" || task.id === "WF-012";
  return `
    <div class="cx-note-editor">
      <div class="cx-product-header"><span class="cx-product-title">${browse ? "Notes Browse" : "Design Contract Notes"}</span><span>${after ? "Saved" : "Current"}</span></div>
      <div class="cx-note-paper">
        <h3># ${geometry ? "Heading glyph proof" : "Consistency review"}</h3>
        <p>${browse ? (after ? "Destination · Open in Notes Window" : "Destination inferred from the host") : after ? "From Notes · Design Contract Notes · pending for next turn" : "Generic Agent handoff · scope not named"}</p>
        <p>${after ? "Selection, scroll, dirty state, aliases, and draft remain intact." : "Cross-window transitions can hide scope and per-item acceptance."}</p>
      </div>
      ${footer(after ? browse ? [["Open in Notes Window", ["↵"]], ["Back", ["Esc"]]] : [[task.id === "WF-014" ? "Add Note" : "Add to Agent Chat", ["⌘", "↵"]], ["Actions", ["⌘", "K"]]] : [["Agent", ["⌘", "↵"]], ["Actions", ["⌘", "K"]]])}
    </div>`;
}

function renderDictation(task, phase) {
  const after = phase === "after";
  const escape = task.id === "SAFE-002";
  const recovery = task.id === "WF-022";
  const history = task.id === "WF-024";
  if (history) {
    return `
      <div class="cx-product-header"><span class="cx-product-title">Dictation History</span><span>${after ? "Showing 100 of 147" : "147 dictations"}</span></div>
      <section class="sk-list"><div class="sk-section-slot" data-first="true"><div class="sk-section-header">RECENT</div></div>${row("Send the revised agenda", "Agent Chat · 1.2s · Jul 31", true)}${row("Review the release notes", "Today · 2.4s · Jul 30")}${row(after ? "Load More" : "Older results not loaded", after ? "47 dictations remaining" : "")}</section>
      ${footer(after ? [["Paste", ["↵"]], ["Copy", ["⌘", "↵"]], ["Actions", ["⌘", "K"]]] : [["Paste", ["↵"]], ["AI", ["⌃", "⌘", "A"]], ["Delete", ["⌘", "⌫"]]])}`;
  }
  return `
    <div class="cx-dictation">
      <div class="cx-dictation-capsule">
        <div class="cx-product-chip-row">
          ${(after ? ["App", "Today", "Quick AI", "Agent Chat"] : ["Paste ↵", "Today ↵", "Ask ↵", "Send ↵"]).map((label, index) => `<span class="cx-product-chip" data-role="destination" data-selected="${index === 0}">${label}</span>`).join("")}
        </div>
        <div class="cx-waveform" aria-hidden="true">${"<i></i>".repeat(18)}</div>
        <div class="cx-transcript">${recovery ? (after ? "Your transcript is safe. Choose another destination or copy it." : "Error: target window unavailable") : escape ? (after ? "Discard dictation? The transcript remains available." : "At 0:03, Escape closes immediately.") : after ? "Selected destination: Mail · frozen for this session" : "Target badge follows the current frontmost app"}</div>
        ${footer(after ? escape ? [["Continue", ["Esc"]], ["Discard", ["⌫"]]] : recovery ? [["Choose Destination", ["↵"]], ["Actions", ["⌘", "K"]], ["Hide", ["Esc"]]] : [["Paste", ["↵"]], ["Discard…", ["Esc"]]] : [["Cancel", ["Esc"]]])}
      </div>
    </div>`;
}

function renderGeometry(task, phase) {
  const after = phase === "after";
  const settings = task.id === "GEO-006" || task.id === "GEO-007";
  const metrics = settings
    ? [["Primary action", after ? "Open" : "Run / Open", 70], ["First section slot", after ? "themed 28" : "legacy 26", 45], ["Icon policy", after ? "Iconless" : "12 hints → 0 icons", 84]]
    : [["Row slot", after ? "paint · 44" : "model · 40", 58], ["Footer role", after ? "reservation · 32" : "height · 30", 72], ["Title/body gap", after ? "renderer · 12" : "model · 16", 40]];
  return `
    <div class="cx-product-header"><span class="cx-product-title">${settings ? "Settings" : "Geometry inspector"}</span><span>${after ? "Named roles" : "Numbers only"}</span></div>
    <div class="cx-metric-stack">
      ${metrics.map(([label, value, position]) => `<div class="cx-metric"><span>${label}</span><div class="cx-metric__bar" style="--cx-metric-position:${position}%"></div><code>${value}</code></div>`).join("")}
    </div>
    ${settings ? footer(after ? [["Open", ["↵"]], ["Actions", ["⌘", "K"]]] : [["Run", ["↵"]], ["Actions", ["⌘", "K"]]]) : footer(after ? [["Compare roles", ["↵"]]] : [["Mismatch?", ["?"]]])}`;
}

function renderAccessibility(task, phase) {
  const after = phase === "after";
  const labels = task.id === "PF-006"
    ? [["Line box", after ? "measured" : "unknown"], ["Glyph bounds", after ? "clipped" : "not exposed"], ["Classification", after ? "EVALUABLE_FAIL" : "screenshot"]]
    : task.id === "PF-007"
      ? [["Semantic node", after ? "open-actions" : "label only"], ["AX peer", after ? "matched host" : "unknown"], ["Activation", after ? "postcondition verified" : "key dispatched"]]
      : [["Selected bounds", after ? "43 / 44 visible" : "unknown"], ["Safe viewport", after ? "footer excluded" : "viewport only"], ["Classification", after ? "EVALUABLE_FAIL" : "ok?"]];
  return `
    <div class="cx-product-header"><span class="cx-product-title">${task.id === "PF-006" ? "Text fit receipt" : task.id === "PF-007" ? "One action, four truths" : "Viewport receipt"}</span><span>${after ? "Inspectable proof" : "Pixel-only evidence"}</span></div>
    <div class="cx-proof-stack">
      ${labels.map(([label, value], index) => `<div class="cx-proof-card" data-state="${after ? (index === 2 ? "pass" : "blocked") : "blocked"}"><strong>${label}</strong>${value}</div>`).join("")}
    </div>
    <div class="cx-product-body">${after ? `<div class="cx-metric-stack"><div class="cx-metric"><span>Product bounds</span><div class="cx-metric__bar" style="--cx-metric-position:72%"></div><code>logical px</code></div></div>` : `<div class="cx-state-card"><p>A selected ID and a visible screenshot do not prove clipping, focus, activation, or safe visibility.</p></div>`}</div>
    ${footer(after ? [["Inspect receipt", ["↵"]], ["Toggle layers", ["1", "–", "6"]]] : [["Capture", ["⌘", "S"]]])}`;
}

function renderGovernance(task, phase) {
  const after = phase === "after";
  let rows;
  if (task.id === "GOV-003") {
    rows = after
      ? [["Authored type", "AlphaByte", "0x32 · byte 50"], ["Normalized", "Resolved color", "0.1960784314"], ["Raster delta", "None", "exact-byte serialization"]]
      : [["Rust type", "f32", "50.0"], ["Unit", "Unknown", "byte or percent?"], ["Resolved", "CSS", "19.6078%"]];
  } else if (task.id === "GOV-004") {
    rows = after
      ? [["Notes Browse", "Resolved", "src/render_builtins/notes_browse.rs"], ["Validator", "Current path", "near matches on failure"], ["Historical", "Explicit", "not current ownership"]]
      : [["Notes Browse", "Missing", "src/notes_browse.rs"], ["Validator", "None", "plausible path appears valid"], ["Near match", "Unknown", "not surfaced"]];
  } else if (task.id === "GOV-007") {
    rows = after
      ? [["Disposition", "Documentation only", "capsule veil 0.0"], ["Protected proof", "Required", "unchanged anti-drift suite"], ["Product delta", "None", "no retune"]]
      : [["Documentation", "0.80", "disagrees"], ["Source + test", "0.0", "current"], ["Decision", "Blocked", "until proof"]];
  } else {
    rows = after
      ? [["Lifecycle", "Owned · open", "task + owner + blocker"], ["Removal", "Proof-gated", "receipt + condition"], ["Product delta", "None", "source-derived regeneration"]]
      : [["Classification", "Unrepresented", "conflict or façade"], ["Owner", "Unknown", "plausible file name"], ["Closure", "None", "count mistaken for progress"]];
  }
  return `
    <div class="cx-product-header"><span class="cx-product-title">Contract ledger</span><span>${escapeHtml(task.id)}</span></div>
    <div class="cx-ledger">${rows.map(([name, state, detail]) => `<div class="cx-ledger-row"><strong>${name}</strong><span>${state}</span><code>${detail}</code></div>`).join("")}</div>
    ${footer(after ? [["Inspect source", ["↵"]], ["Copy path", ["⌘", "C"]]] : [["Looks plausible", ["?"]]])}`;
}

function bindDecisions(group, review) {
  $$('[data-decision-form]').forEach((form) => {
    const taskId = form.dataset.taskId;
    $$('input[type="radio"]', form).forEach((radio) => {
      radio.addEventListener("change", () => {
        writeDecision(review, group.id, taskId, { verdict: radio.value });
        const nav = $(`[data-nav-task="${CSS.escape(taskId)}"]`);
        if (nav) nav.dataset.reviewed = "true";
        updateGroupProgress(group, review);
        announce(`${taskId} marked ${radio.value}.`);
      });
    });
    const note = $('[data-decision-note]', form);
    let timer;
    note.addEventListener("input", () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        writeDecision(review, group.id, taskId, { note: note.value });
        announce(`Saved note for ${taskId}.`);
      }, 250);
    });
    $('[data-clear-decision]', form).addEventListener("click", () => {
      $$('input[type="radio"]', form).forEach((radio) => { radio.checked = false; });
      note.value = "";
      writeDecision(review, group.id, taskId, { verdict: null, note: "" });
      const nav = $(`[data-nav-task="${CSS.escape(taskId)}"]`);
      if (nav) nav.dataset.reviewed = "false";
      updateGroupProgress(group, review);
      announce(`Cleared decision for ${taskId}.`);
    });
  });
}

function updateGroupProgress(group, review) {
  const decided = group.taskRecords.filter((task) => decisionFor(review, group.id, task.id).verdict).length;
  const target = $('[data-group-progress]');
  if (target) target.textContent = `${decided}/${group.taskRecords.length}`;
}

function bindCompareControls() {
  const page = $('.cx-page--group');
  const scrub = $('[data-scrubber]');
  const scrubControl = $('.cx-scrub-control');
  page.dataset.viewMode = "split";
  $$('[data-view-mode]').forEach((button) => {
    button.addEventListener("click", () => {
      const mode = button.dataset.viewMode;
      page.dataset.viewMode = mode;
      $$('[data-view-mode]').forEach((candidate) => candidate.setAttribute("aria-pressed", String(candidate === button)));
      scrubControl.hidden = mode !== "overlay";
      announce(`Comparison mode: ${button.textContent.trim()}.`);
    });
  });
  scrub.addEventListener("input", () => {
    page.style.setProperty("--cx-scrub", `${scrub.value}%`);
    scrub.nextElementSibling.value = `${scrub.value}%`;
  });
}

function bindTaskFilter() {
  const input = $('[data-task-filter]');
  input.addEventListener("input", () => {
    const query = input.value.trim().toLowerCase();
    let visible = 0;
    $$('[data-scene-id]').forEach((scene) => {
      scene.hidden = Boolean(query) && !scene.dataset.search.includes(query);
      if (!scene.hidden) visible += 1;
    });
    announce(`${visible} tasks visible.`);
  });
}

function updateLayoutReceipt() {
  const root = document.documentElement;
  root.dataset.horizontalOverflow = String(root.scrollWidth > root.clientWidth + 1);
  root.dataset.renderReady = "true";
}

async function runBrowserSelfTest() {
  if (new URLSearchParams(window.location.search).get("selfTest") !== "1") return;
  const root = document.documentElement;
  const original = localStorage.getItem(STORAGE_KEY);
  try {
    const firstForm = $('[data-decision-form]');
    const taskId = firstForm.dataset.taskId;
    const approve = $('input[value="approve"]', firstForm);
    approve.click();
    const note = $('[data-decision-note]', firstForm);
    note.value = "browser self-test";
    note.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise((resolve) => setTimeout(resolve, 320));

    $('[data-view-mode="overlay"]').click();
    const scrub = $('[data-scrubber]');
    scrub.value = "73";
    scrub.dispatchEvent(new Event("input", { bubbles: true }));

    const filter = $('[data-task-filter]');
    filter.value = taskId;
    filter.dispatchEvent(new Event("input", { bubbles: true }));

    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    const decision = stored.groups?.[$('#consistency-app').dataset.groupId]?.tasks?.[taskId];
    const visibleScenes = $$('[data-scene-id]').filter((scene) => !scene.hidden).length;
    const passed = decision?.verdict === "approve"
      && decision?.note === "browser self-test"
      && $('.cx-page--group').dataset.viewMode === "overlay"
      && $('.cx-page--group').style.getPropertyValue("--cx-scrub") === "73%"
      && visibleScenes === 1;
    root.dataset.selfTest = passed ? "pass" : "fail";
  } catch (error) {
    root.dataset.selfTest = "fail";
    root.dataset.selfTestError = error.message;
  } finally {
    if (original === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, original);
  }
}

async function main() {
  const manifest = await loadManifest();
  const review = loadReview();
  const page = document.documentElement.dataset.consistencyPage;
  if (page === "index") {
    renderIndex(manifest, review);
  } else {
    const groupId = $("#consistency-app").dataset.groupId;
    const group = manifest.groups.find((item) => item.id === groupId);
    if (!group) throw new Error(`Unknown consistency group: ${groupId}`);
    renderGroup(manifest, group, review);
  }
  bindGlobalActions(manifest, review);
  await document.fonts?.ready;
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  updateLayoutReceipt();
  window.addEventListener("resize", updateLayoutReceipt, { passive: true });
  await runBrowserSelfTest();
}

main().catch((error) => {
  console.error(error);
  const app = $("#consistency-app");
  if (app) app.innerHTML = `<section class="cx-state-card"><h1>Review explorer could not load</h1><p>${escapeHtml(error.message)}</p></section>`;
});

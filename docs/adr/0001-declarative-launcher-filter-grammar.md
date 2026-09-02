# Declarative Launcher Filter Grammar

Launcher filters use declarative, order-independent trailing-colon heads so users can constrain search predictably while keeping `:` as a transient discovery trigger. Source heads such as `files:`/`f:`, `commands:`/`cmd:`, and `conversations:`/`ai:` are valueless universe selectors; property heads such as `type:`, `tag:`, and `shortcut:` require values and use scoped pickers.

We chose this over committed `:f` tokens, suffix add/subtract forms like `f+`, and procedural "filter the current visible list" semantics because async root sources must not change the meaning of an already-typed query. Multiple positive source heads are additive, leading-minus heads exclude, repeated values for one property are OR, different property heads are AND, and exclusion wins conflicts.

Filtered empty, unavailable, invalid, incomplete, and contradiction states remain structured states: they show filter-aware copy and explicit recovery actions rather than broadening to global search or executing generic fallbacks. Filter grammar only constrains retrieval and selection; asking an AI model requires explicit AI intent through an action or command, not inference from filter tokens.

## Async publication and interaction contract

Query meaning belongs to an immutable computed-query revision, including its parsed grammar and retrieval scope. A raw input change retires the previous consumer immediately, before coalesced computation; returning to the same text does not revive the old consumer. Exact no-op input updates and caret motion do not start a new query. Provider work identity and permission to publish into the current query are separate: compatible work can be explicitly reattached at an input boundary, never opportunistically accepted because its text happens to match.

Publication policy is source-specific. Implicit global Files results warm the next-query cache; explicit Files and directory results may publish into their current query. Other admitted publishing sources may replace committed rows without changing the query's meaning. Each publication captures interaction state before source mutation and commits rows, selection, preview/action subject, and revision together.

Automatic selection follows the first eligible result. Deliberate keyboard, pointer, or semantic selection anchors the selected stable identity, including an explicit selection of the already-first row. If that identity disappears, fall back to the first eligible row and return to automatic intent. No eligible row means no selection or activation. Keyboard focus and the user-controlled viewport are independent of row selection. A late publication may reorder rows, but a stale click or agent target must never execute a different row now occupying an old position.

Agent evidence joins query revision → provider run and consumer attachment → committed rows → selection intent/subject → completed GPUI frame. State inspection alone is not paint proof; forced redraw is not proof that a publication scheduled a frame. The bounded owned search recipes are the executable acceptance inventory; unavailable capabilities remain uncovered, never a passing substitute.

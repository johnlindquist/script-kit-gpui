# Form and Fields SDK Parity Report

Reviewed: 2026-08-21.

`fields()` and `form()` are both implemented. The SDK sends their prompt
messages, the Rust prompt router dispatches `Message::Fields` to `ShowFields`,
and the host owns the corresponding prompt handler. Earlier claims that
`fields()` fell through to an unhandled-message branch are obsolete.

## Current SDK contract

| API | SDK return contract | Current boundary |
| --- | --- | --- |
| `fields(definitions)` | `Promise<string[]>`, preserving definition order and defaults | Field definitions are transported to the native fields prompt. |
| `form(html)` | `Promise<Record<string, string>>` | Supported HTML form controls are parsed into native prompt controls. |

`FieldDef.type` currently accepts `text`, `password`, `email`, `number`,
`date`, `time`, `datetime-local`, `month`, `week`, `search`, `url`, `tel`, and
`color`.

A typed SDK field is not proof of a specialized native control. Date/time,
month/week, search, URL, telephone, and color variants can retain the host's
shared text-field treatment; a native calendar, color picker, or browser-style
validation requires separate direct runtime evidence. `range`, `file`,
`checkbox`, `radio`, and `hidden` are not advertised as `FieldDef` capabilities.
Use supported `form(html)` controls where their actual native implementation is
documented.

## Executable SDK coverage

```bash
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter 'fields-basic|fields-datetime|form-specialized'
bun run scripts/check-sdk-types.ts
```

- `test-fields-basic.ts` checks string labels, basic field types, result order,
  and prefilled values.
- `test-fields-datetime.ts` checks date, time, datetime-local, month, week,
  combined definitions, and the search type.
- `test-form-specialized.ts` checks URL, search, telephone, color, textarea,
  and mixed-definition request/response contracts.

These checks run in the isolated SDK auto-submit harness. They do not launch
the application, capture the desktop, focus windows, inject input, or claim
native rendering coverage. Prompt rendering, native control behavior, and
keyboard/accessibility semantics remain separate host-runtime proof obligations.

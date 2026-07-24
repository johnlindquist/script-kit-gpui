//! App-level adapters for the provider-independent AI reliability domain.
//!
//! Raw provider, process, and protocol failures stop at this boundary. Callers
//! receive an exhaustive domain failure plus presentation keys and a reference
//! to redacted secondary diagnostics; primary UI never receives the raw input.

mod classify;
mod diagnostics;

#[cfg(test)]
mod tests;

pub use classify::{
    classify_process_failure, classify_protocol_failure, classify_provider_failure,
    AppFailureRecord, FailureContext, FailurePresentationInput, ProcessFailureFacts,
    ProtocolFailureFacts,
};
pub use diagnostics::{redact_diagnostic, DiagnosticVault, RedactedDiagnostic};

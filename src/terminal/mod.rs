//! Terminal integration module for Script Kit GPUI.
//! Terminal module for Script Kit GPUI
//!
//! This module provides embedded terminal functionality using Alacritty's terminal
//! emulator backend and portable-pty for cross-platform PTY support. It enables
//! Script Kit to swap prompts into full terminal sessions via `await term("command")`.
#![allow(dead_code)]
//!
//! # Architecture
//!
//! The terminal integration consists of three main components:
//!
//! - **PTY Manager** ([`PtyManager`]): Handles cross-platform pseudo-terminal creation
//!   and lifecycle management using `portable-pty`.
//!
//! - **Terminal Handle** ([`TerminalHandle`]): Wraps Alacritty's terminal emulator,
//!   managing the terminal grid, parsing escape sequences, and maintaining state.
//!
//! - **Theme Adapter** ([`ThemeAdapter`]): Converts Script Kit's theme system to
//!   Alacritty's color configuration for seamless visual integration.
//!
//!
//! # Features
//!
//! - Full VT100/xterm escape sequence support via Alacritty
//! - Cross-platform PTY support (macOS, Linux, Windows)
//! - Theme integration with Script Kit's color system
//! - Event-driven architecture for GPUI integration

pub mod alacritty;
pub mod pty;
pub mod theme_adapter;

// Re-export main types for convenient access
pub use alacritty::{CellAttributes, TerminalContent, TerminalHandle};

/// Events emitted by the terminal for GPUI integration.
///
/// These events are used to communicate terminal state changes to the
/// rendering layer and allow Script Kit to respond to terminal activity.
///
/// # Event Flow
///
/// ```text
/// PTY Output → Terminal Parser → TerminalEvent → GPUI Render
/// ```
///
/// The terminal continuously parses PTY output and emits these events
/// which should be polled in the GPUI render loop.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TerminalEvent {
    /// Terminal produced output that should be rendered.
    ///
    /// This includes both regular text and control sequences that
    /// have been processed into renderable content.
    Output(String),

    /// Terminal bell (BEL character, \x07) was received.
    ///
    /// Applications can respond with visual or audio feedback.
    Bell,

    /// Terminal title changed via OSC escape sequence.
    ///
    /// This is typically set by shells to show the current directory
    /// or running command.
    Title(String),

    /// Terminal process exited with the given status code.
    ///
    /// A code of 0 typically indicates success, while non-zero
    /// indicates an error or abnormal termination.
    Exit(i32),
}

impl TerminalEvent {
    /// Returns `true` if this is an [`Exit`](TerminalEvent::Exit) event.
    #[inline]
    pub fn is_exit(&self) -> bool {
        matches!(self, TerminalEvent::Exit(_))
    }

    /// Returns the exit code if this is an [`Exit`](TerminalEvent::Exit) event.
    #[inline]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            TerminalEvent::Exit(code) => Some(*code),
            _ => None,
        }
    }

    /// Returns `true` if this is an [`Output`](TerminalEvent::Output) event.
    #[inline]
    pub fn is_output(&self) -> bool {
        matches!(self, TerminalEvent::Output(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_event_is_exit() {
        assert!(TerminalEvent::Exit(0).is_exit());
        assert!(!TerminalEvent::Bell.is_exit());
        assert!(!TerminalEvent::Output("test".into()).is_exit());
    }

    #[test]
    fn test_terminal_event_exit_code() {
        assert_eq!(TerminalEvent::Exit(0).exit_code(), Some(0));
        assert_eq!(TerminalEvent::Exit(1).exit_code(), Some(1));
        assert_eq!(TerminalEvent::Bell.exit_code(), None);
    }

    #[test]
    fn test_terminal_event_is_output() {
        assert!(TerminalEvent::Output("hello".into()).is_output());
        assert!(!TerminalEvent::Bell.is_output());
        assert!(!TerminalEvent::Exit(0).is_output());
    }
}

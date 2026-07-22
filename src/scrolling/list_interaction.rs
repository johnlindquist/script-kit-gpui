//! Product-neutral interaction policy for list viewports and row pointers.
//!
//! Viewport input is intentionally tracked separately from keyboard/mouse
//! modality: wheel and scrollbar movement do not synthesize pointer movement.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ListViewportInputSource {
    Keyboard,
    Click,
    Wheel,
    Momentum,
    Scrollbar,
    Filter,
    #[default]
    Refresh,
}

impl ListViewportInputSource {
    pub(crate) fn from_event(event: &gpui::ScrollWheelEvent) -> Self {
        if event.momentum_phase != gpui::ScrollPhase::None {
            Self::Momentum
        } else {
            Self::Wheel
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Click => "click",
            Self::Wheel => "wheel",
            Self::Momentum => "momentum",
            Self::Scrollbar => "scrollbar",
            Self::Filter => "filter",
            Self::Refresh => "refresh",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ListPointerPolicy {
    pub(crate) hovered_index: Option<usize>,
    pub(crate) suppress_hover_until_pointer_move: bool,
}

impl ListPointerPolicy {
    pub(crate) fn begin_viewport_scroll(&mut self) {
        self.suppress_hover_until_pointer_move = true;
        self.hovered_index = None;
    }

    pub(crate) fn enter_keyboard_mode(&mut self) {
        self.suppress_hover_until_pointer_move = true;
        self.hovered_index = None;
    }

    pub(crate) fn note_pointer_move(&mut self, row: usize) {
        self.suppress_hover_until_pointer_move = false;
        self.hovered_index = Some(row);
    }

    /// GPUI hover-enter can be synthesized when content moves beneath a
    /// stationary pointer. Only a real pointer move may establish hover.
    pub(crate) fn note_hover_change(&mut self, row: usize, hovered: bool) {
        if self.suppress_hover_until_pointer_move {
            return;
        }
        if hovered {
            self.hovered_index = Some(row);
        } else if self.hovered_index == Some(row) {
            self.hovered_index = None;
        }
    }

    pub(crate) fn note_pointer_click(&mut self, row: usize) {
        self.suppress_hover_until_pointer_move = false;
        self.hovered_index = Some(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_clears_hover_and_arms_suppression() {
        let mut policy = ListPointerPolicy {
            hovered_index: Some(4),
            suppress_hover_until_pointer_move: false,
        };
        policy.begin_viewport_scroll();
        assert_eq!(policy.hovered_index, None);
        assert!(policy.suppress_hover_until_pointer_move);
    }

    #[test]
    fn synthetic_hover_enter_is_ignored_until_real_pointer_move() {
        let mut policy = ListPointerPolicy {
            hovered_index: None,
            suppress_hover_until_pointer_move: true,
        };
        policy.note_hover_change(3, true);
        assert_eq!(policy.hovered_index, None);

        policy.note_pointer_move(3);
        assert_eq!(policy.hovered_index, Some(3));
        assert!(!policy.suppress_hover_until_pointer_move);
    }

    #[test]
    fn pointer_leave_clears_only_the_active_hover() {
        let mut policy = ListPointerPolicy {
            hovered_index: Some(2),
            suppress_hover_until_pointer_move: false,
        };
        policy.note_hover_change(1, false);
        assert_eq!(policy.hovered_index, Some(2));
        policy.note_hover_change(2, false);
        assert_eq!(policy.hovered_index, None);
    }

    #[test]
    fn keyboard_clears_hover_and_suppresses_until_pointer_move() {
        let mut policy = ListPointerPolicy {
            hovered_index: Some(1),
            suppress_hover_until_pointer_move: false,
        };
        policy.enter_keyboard_mode();
        assert_eq!(policy.hovered_index, None);
        assert!(policy.suppress_hover_until_pointer_move);
    }

    #[test]
    fn click_establishes_pointer_hover_without_prior_move() {
        let mut policy = ListPointerPolicy {
            hovered_index: None,
            suppress_hover_until_pointer_move: true,
        };
        policy.note_pointer_click(7);
        assert_eq!(policy.hovered_index, Some(7));
        assert!(!policy.suppress_hover_until_pointer_move);
    }

    #[test]
    fn momentum_is_distinct_from_direct_wheel_input() {
        let direct = gpui::ScrollWheelEvent::default();
        assert_eq!(
            ListViewportInputSource::from_event(&direct),
            ListViewportInputSource::Wheel
        );

        let momentum = gpui::ScrollWheelEvent {
            momentum_phase: gpui::ScrollPhase::Changed,
            ..Default::default()
        };
        assert_eq!(
            ListViewportInputSource::from_event(&momentum),
            ListViewportInputSource::Momentum
        );
    }
}

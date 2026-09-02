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

/// Launcher intent is independent from the selected row's changing rank.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum MainMenuSelectionIntent {
    #[default]
    AutomaticTop,
    AutomaticAnchor {
        stable_key: String,
    },
    ExplicitAnchor {
        stable_key: String,
    },
}

impl MainMenuSelectionIntent {
    pub(crate) fn reconcile<'a>(
        &mut self,
        rows: impl Iterator<Item = (usize, &'a str, bool)>,
    ) -> (Option<usize>, bool) {
        // Once shown, even the default selection belongs to this query.
        // A later, better-ranked result must not steal the user's Enter target.
        for (index, key, eligible) in rows {
            if !eligible {
                continue;
            }
            match self {
                Self::AutomaticTop => {
                    *self = Self::AutomaticAnchor {
                        stable_key: key.to_owned(),
                    };
                    return (Some(index), false);
                }
                Self::AutomaticAnchor { stable_key } | Self::ExplicitAnchor { stable_key }
                    if stable_key == key =>
                {
                    return (Some(index), false)
                }
                _ => {}
            }
        }
        // A vanished target is not permission to activate an unrelated result.
        // Keep its identity until a new query or deliberate selection replaces it.
        (None, !matches!(self, Self::AutomaticTop))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MainMenuViewportIntent {
    #[default]
    FollowSelection,
    UserControlled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainMenuRefreshViewportPolicy {
    ResetToTop,
    Preserve { reveal_selection: bool },
}

impl MainMenuViewportIntent {
    /// True means this input retires any previously deferred viewport work.
    pub(crate) fn note_input(&mut self, source: ListViewportInputSource) -> bool {
        match source {
            ListViewportInputSource::Wheel
            | ListViewportInputSource::Momentum
            | ListViewportInputSource::Scrollbar => {
                *self = Self::UserControlled;
            }
            ListViewportInputSource::Filter => *self = Self::FollowSelection,
            ListViewportInputSource::Keyboard
            | ListViewportInputSource::Click
            | ListViewportInputSource::Refresh => return false,
        }
        true
    }

    pub(crate) fn refresh_policy(
        self,
        automatic: bool,
        same_query: bool,
        selected_was_visible: bool,
    ) -> MainMenuRefreshViewportPolicy {
        if !same_query || (automatic && self == Self::FollowSelection) {
            MainMenuRefreshViewportPolicy::ResetToTop
        } else {
            MainMenuRefreshViewportPolicy::Preserve {
                reveal_selection: !automatic
                    && self == Self::FollowSelection
                    && selected_was_visible,
            }
        }
    }
}

/// Resolve a keyboard gesture before it is allowed to establish selection intent.
pub(crate) fn main_menu_navigation_target(
    eligible: impl DoubleEndedIterator<Item = usize> + Clone,
    selected_index: usize,
    delta: i32,
    unarmed: bool,
) -> Option<usize> {
    if delta == 0 {
        return None;
    }
    let first = eligible.clone().next()?;
    if unarmed {
        return (delta > 0).then_some(first);
    }
    if delta < 0 && selected_index <= first {
        return None;
    }
    let mut target = selected_index;
    let steps = delta.unsigned_abs() as usize;
    if delta > 0 {
        for index in eligible.filter(|index| *index > selected_index).take(steps) {
            target = index;
        }
    } else {
        for index in eligible
            .rev()
            .filter(|index| *index < selected_index)
            .take(steps)
        {
            target = index;
        }
    }
    Some(target)
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
    fn keyboard_ignores_up_at_top_and_only_down_arms_empty_subsearch() {
        let eligible = [1, 4, 8];
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 1, -1, false),
            None
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 1, -1, true),
            None
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 1, 1, true),
            Some(1)
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 1, 1, false),
            Some(4)
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 8, -1, false),
            Some(4)
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 1, 10, false),
            Some(8)
        );
        assert_eq!(
            main_menu_navigation_target(eligible.into_iter(), 8, 1, false),
            Some(8)
        );
        assert_eq!(
            main_menu_navigation_target([].into_iter(), 0, 1, true),
            None
        );
    }

    #[test]
    fn automatic_and_explicit_selection_keep_identity_after_higher_arrivals() {
        let rows = [(1, "new", true), (3, "previous", true)];
        let mut automatic = MainMenuSelectionIntent::AutomaticTop;
        assert_eq!(automatic.reconcile(std::iter::empty()), (None, false));
        assert_eq!(
            automatic.reconcile([(1, "previous", true)].into_iter()),
            (Some(1), false)
        );
        assert_eq!(automatic.reconcile(rows.into_iter()), (Some(3), false));
        let mut explicit = MainMenuSelectionIntent::ExplicitAnchor {
            stable_key: "previous".into(),
        };
        assert_eq!(explicit.reconcile(rows.into_iter()), (Some(3), false));
        assert!(matches!(
            explicit,
            MainMenuSelectionIntent::ExplicitAnchor { .. }
        ));
        automatic = MainMenuSelectionIntent::default();
        assert_eq!(automatic.reconcile(rows.into_iter()), (Some(1), false));
    }

    #[test]
    fn removed_or_ineligible_anchor_does_not_select_an_unrelated_result() {
        for rows in [
            vec![(1, "next", true)],
            vec![(1, "previous", false), (3, "next", true)],
            vec![],
        ] {
            for mut intent in [
                MainMenuSelectionIntent::AutomaticAnchor {
                    stable_key: "previous".into(),
                },
                MainMenuSelectionIntent::ExplicitAnchor {
                    stable_key: "previous".into(),
                },
            ] {
                let original = intent.clone();
                assert_eq!(intent.reconcile(rows.iter().copied()), (None, true));
                assert_eq!(intent, original);
                assert_eq!(
                    intent.reconcile([(2, "new-top", true)].into_iter()),
                    (None, true)
                );
            }
        }
    }

    #[test]
    fn repeated_wheel_and_refresh_keep_independent_viewport_ownership() {
        let mut viewport = MainMenuViewportIntent::FollowSelection;
        assert_eq!(
            viewport.refresh_policy(true, true, true),
            MainMenuRefreshViewportPolicy::ResetToTop
        );
        for source in [
            ListViewportInputSource::Wheel,
            ListViewportInputSource::Momentum,
            ListViewportInputSource::Scrollbar,
        ] {
            assert!(viewport.note_input(source));
            assert!(viewport.note_input(source));
            assert!(!viewport.note_input(ListViewportInputSource::Refresh));
            assert_eq!(viewport, MainMenuViewportIntent::UserControlled);
            for automatic in [true, false] {
                assert_eq!(
                    viewport.refresh_policy(automatic, true, true),
                    MainMenuRefreshViewportPolicy::Preserve {
                        reveal_selection: false
                    }
                );
            }
        }
        assert!(viewport.note_input(ListViewportInputSource::Filter));
        assert_eq!(viewport, MainMenuViewportIntent::FollowSelection);
    }

    #[test]
    fn refresh_only_reveals_previously_visible_explicit_selection_without_user_scroll() {
        let viewport = MainMenuViewportIntent::FollowSelection;
        assert_eq!(
            viewport.refresh_policy(false, true, true),
            MainMenuRefreshViewportPolicy::Preserve {
                reveal_selection: true
            }
        );
        assert_eq!(
            viewport.refresh_policy(false, true, false),
            MainMenuRefreshViewportPolicy::Preserve {
                reveal_selection: false
            }
        );
        assert_eq!(
            viewport.refresh_policy(false, false, true),
            MainMenuRefreshViewportPolicy::ResetToTop
        );
    }

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

    #[test]
    fn builtin_native_uniform_scroll_stationary_pointer_and_click_matrix() {
        for surface in [
            "app_launcher",
            "browser_tabs",
            "current_app_commands",
            "tips",
            "window_switcher",
        ] {
            let mut policy = ListPointerPolicy {
                hovered_index: Some(2),
                suppress_hover_until_pointer_move: false,
            };
            policy.begin_viewport_scroll();
            policy.note_hover_change(6, true);
            assert_eq!(
                policy.hovered_index, None,
                "{surface}: content moving beneath a stationary pointer must not hover"
            );

            policy.note_pointer_move(6);
            assert_eq!(
                policy.hovered_index,
                Some(6),
                "{surface}: real pointer move"
            );

            policy.begin_viewport_scroll();
            policy.note_pointer_click(4);
            assert_eq!(
                policy.hovered_index,
                Some(4),
                "{surface}: click establishes row"
            );
            assert!(!policy.suppress_hover_until_pointer_move);
        }
    }
}

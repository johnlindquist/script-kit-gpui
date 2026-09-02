use super::*;

use crate::effects::BackgroundEffect;

impl ScriptListApp {
    /// Switch the background effect, restart its clock, manage the frame
    /// ticker, and persist the choice.
    pub(crate) fn set_background_effect(
        &mut self,
        effect: Option<BackgroundEffect>,
        cx: &mut Context<Self>,
    ) {
        if self.background_effect != effect { self.mark_main_data_changed(); }
        self.mark_main_presentation_changed();
        self.background_effect = effect;
        if effect.is_some() {
            self.background_effect_started_at = Some(std::time::Instant::now());
            if self._background_effect_ticker.is_none() {
                self.start_background_effect_ticker(cx);
            }
        } else {
            self.background_effect_started_at = None;
            self._background_effect_ticker = None;
        }

        let mut prefs = if self.main_services.is_production() { crate::config::load_user_preferences() } else { self.main_preferences.clone() };
        // "Effect Off" persists the explicit "off" sentinel — a bare absent
        // key now means "use the install default" (Starfield).
        let slug = Some(BackgroundEffect::persisted_slug(effect));
        if prefs.effects.background != slug {
            prefs.effects.background = slug;
            if !self.main_services.is_production() { self.main_preferences.effects = prefs.effects; }
            else if let Err(err) = crate::config::save_user_preferences(&prefs) {
                logging::log(
                    "EFFECTS",
                    &format!("Failed to persist background effect: {err}"),
                );
            }
        }
        cx.notify();
    }

    /// Re-render while an effect is active so its time uniform advances:
    /// ~30fps while the main window is visible, with a slow keep-alive poll
    /// while it is hidden so the effect resumes without burning frames in
    /// the background.
    pub(crate) fn start_background_effect_ticker(&mut self, cx: &mut Context<Self>) {
        let owned = self.main_services.host_policy().is_hidden();
        self._background_effect_ticker = Some(cx.spawn(async move |this, cx| loop {
            let interval = if crate::is_main_window_visible() || owned {
                33
            } else {
                500
            };
            cx.background_executor()
                .timer(std::time::Duration::from_millis(interval))
                .await;
            let should_stop = this
                .update(cx, |view, cx| {
                    if view.background_effect.is_none() {
                        return true;
                    }
                    if crate::is_main_window_visible() || owned {
                        cx.notify();
                    }
                    false
                })
                .unwrap_or(true);
            if should_stop {
                break;
            }
        }));
    }
}

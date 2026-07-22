use super::*;
use gpui::Context;

impl ScriptListApp {
    pub(crate) fn warm_quick_terminal_pty(&mut self, cx: &mut Context<Self>) {
        if std::env::var("SCRIPT_KIT_DISABLE_QUICK_TERMINAL_WARM_PTY")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            tracing::debug!(
                event = "quick_terminal_warm_pty_skipped",
                reason = "disabled_by_env",
            );
            return;
        }

        if self.quick_terminal_warm_pty.is_some() || self.quick_terminal_warm_inflight {
            return;
        }

        self.quick_terminal_warm_inflight = true;
        let app = cx.entity().downgrade();
        let theme = std::sync::Arc::clone(&self.theme);
        cx.spawn(async move |_this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::terminal::TerminalHandle::new_with_theme(
                        QUICK_TERMINAL_INITIAL_COLS,
                        QUICK_TERMINAL_INITIAL_ROWS,
                        &theme,
                    )
                })
                .await;

            cx.update(|cx| {
                let Some(app) = app.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.quick_terminal_warm_inflight = false;
                    match result {
                        Ok(handle) => {
                            this.quick_terminal_warm_pty = Some(handle);
                            this.quick_terminal_warm_created_at = Some(std::time::Instant::now());
                            tracing::debug!(
                                event = "quick_terminal_warm_pty_ready",
                                cols = QUICK_TERMINAL_INITIAL_COLS,
                                rows = QUICK_TERMINAL_INITIAL_ROWS,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "quick_terminal_warm_pty_failed",
                                error = %error,
                            );
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(crate) fn take_quick_terminal_warm_pty(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crate::terminal::TerminalHandle> {
        let created_at = self.quick_terminal_warm_created_at.take();
        let mut handle = self.quick_terminal_warm_pty.take()?;
        let too_old = created_at
            .map(|created_at| created_at.elapsed() > QUICK_TERMINAL_WARM_TTL)
            .unwrap_or(false);

        if too_old || !handle.is_alive() {
            if too_old {
                tracing::debug!(event = "quick_terminal_warm_pty_expired");
            } else {
                tracing::warn!(event = "quick_terminal_warm_pty_dead");
            }
            let _ = handle.kill();
            self.warm_quick_terminal_pty(cx);
            return None;
        }

        Some(handle)
    }

    pub(crate) fn clear_quick_terminal_warm_pty(&mut self) {
        self.quick_terminal_warm_inflight = false;
        self.quick_terminal_warm_created_at = None;
        if let Some(mut handle) = self.quick_terminal_warm_pty.take() {
            let _ = handle.kill();
        }
    }
}

impl Drop for ScriptListApp {
    fn drop(&mut self) {
        self.clear_quick_terminal_warm_pty();
    }
}

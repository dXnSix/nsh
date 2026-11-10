//! The app's state (`lifecycle()`) from the app framework's pause and
//! resume. The Tizen platform reports them to GPUI as its one window's
//! activation (`on_active_status_change`, `is_active`) and visibility
//! (hidden while paused): another app, the TV's own UI or standby covers
//! the app; nothing else deactivates the window. Paused, the app is not
//! seen at all: `background`, not the `inactive` a desktop window that lost
//! the focus would give, whatever the order GPUI reports the two in.

use nsh_platform_gpui::Lifecycle;

/// The app's state while the window is active (resumed) or not (paused).
pub(crate) fn of_window(active: bool) -> Lifecycle {
    if active {
        Lifecycle::Active
    } else {
        Lifecycle::Background
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_is_background_and_resume_active() {
        assert_eq!(of_window(false), Lifecycle::Background);
        assert_eq!(of_window(true), Lifecycle::Active);
    }
}

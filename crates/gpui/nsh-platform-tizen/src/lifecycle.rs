//! The app's state (`lifecycle()`) from the app framework's pause and
//! resume. The Tizen platform deactivates its one window when the app is
//! paused (another app, the TV's own UI or standby covers it) and activates
//! it when the app resumes; nothing else deactivates it. Paused, the app is
//! not seen at all: `background`, not the `inactive` a desktop window that
//! lost the focus would give.

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

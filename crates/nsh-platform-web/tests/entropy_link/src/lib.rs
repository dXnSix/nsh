//! Draws entropy through both getrandom releases, so the module must define
//! their custom backend to link.

use nsh_platform_web as _;

/// Fills two buffers, one through each release, and says whether both
/// calls succeeded and returned bytes.
#[unsafe(no_mangle)]
pub extern "C" fn entropy_probe() -> u32 {
    let mut a = [0u8; 16];
    let mut b = [0u8; 16];
    let first = getrandom_v03::fill(&mut a).is_ok();
    let second = getrandom_v04::fill(&mut b).is_ok();
    u32::from(first && second && a != b)
}

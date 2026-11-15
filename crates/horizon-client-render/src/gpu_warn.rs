//! `GpuWarnlistManager`: vanilla matches the GL renderer and vendor strings against
//! `gpu_warnlist.json` and offers an unsupported-graphics warning once. Horizon ships no
//! warnlist, so the small list below names the software rasterisers and known-bad drivers
//! the engine can meet; it is deliberately not a vendor database.
/// Why [`gpu_warning_for`] warned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The adapter is the CPU: WARP, llvmpipe, SwiftShader and the like.
    Software,
    /// The renderer string names a driver known to render the game wrongly.
    KnownBadDriver,
}

/// The adapter as the dialog shows it, and why it warns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub renderer: String,
    pub vendor: String,
    pub reason: Reason,
}

/// `gpu_warnlist.json`'s renderer entries that name something other than a working GPU driver.
const KNOWN_BAD_RENDERERS: &[&str] = &[
    "llvmpipe",                // Mesa's CPU rasteriser
    "softpipe",                // Mesa's older CPU rasteriser
    "swiftshader",             // ANGLE's CPU backend, and standalone
    "microsoft basic render",  // WARP through D3D
    "gdi generic",             // Windows' software OpenGL 1.1 fallback
    "apple software renderer", // macOS' CPU fallback
    "mesa offscreen",          // headless Mesa
    "vmware svga",             // virtualised adapter without 3D acceleration
];

/// The warning for an adapter, if vanilla's warnlist rules would raise one.
pub fn gpu_warning_for(renderer: &str, vendor: &str, software: bool) -> Option<Warning> {
    let named = format!("{renderer} {vendor}").to_ascii_lowercase();
    let reason = if software {
        Reason::Software
    } else if KNOWN_BAD_RENDERERS.iter().any(|bad| named.contains(bad)) {
        Reason::KnownBadDriver
    } else {
        return None;
    };
    Some(Warning {
        renderer: renderer.to_owned(),
        vendor: vendor.to_owned(),
        reason,
    })
}

/// Whether the warning still shows: `skipGpuWarning` suppresses it the way
/// `skipMultiplayerWarning` suppresses the safety screen.
pub fn warning_shown(warning: Option<&Warning>, skip: bool) -> bool {
    warning.is_some() && !skip
}

/// `VideoSettingsScreen.mouseClicked`: turning **Improved Transparency** on raises
/// `UnsupportedGraphicsWarningScreen` for an adapter the warnlist names; turning it
/// off raises nothing, and a saved `skipGpuWarning` keeps the dialog away.
pub fn transparency_toggle_warning(
    warning: Option<Warning>,
    turning_on: bool,
    skip: bool,
) -> Option<Warning> {
    warning.filter(|_| turning_on && !skip)
}

/// [`gpu_warning_for`] for the adapter the renderer found.
pub fn adapter_warning() -> Option<Warning> {
    let adapter = crate::gfx::adapter()?;
    gpu_warning_for(&adapter.name, &adapter.vendor, adapter.kind == "cpu")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_warning_matches_software_bad_drivers_and_skip() {
        // An ordinary discrete adapter warns about nothing.
        assert_eq!(
            gpu_warning_for("NVIDIA GeForce RTX 4070", "NVIDIA", false),
            None
        );
        // A CPU adapter warns even when its renderer string is unfamiliar.
        let software = gpu_warning_for("Mesa", "Mesa", true).unwrap();
        assert_eq!(software.reason, Reason::Software);
        // A known-bad renderer warns without the software flag.
        let bad = gpu_warning_for("SwiftShader Device (Subzero)", "Google", false).unwrap();
        assert_eq!(bad.reason, Reason::KnownBadDriver);
        // "Don't show again" suppresses it.
        assert!(warning_shown(Some(&bad), false));
        assert!(!warning_shown(Some(&bad), true));
    }

    #[test]
    fn improved_transparency_toggle_warns_once() {
        // Turning the option on on a software adapter requests the dialog.
        let software = gpu_warning_for("Mesa", "Mesa", true);
        assert!(transparency_toggle_warning(software.clone(), true, false).is_some());
        // Turning it off requests nothing.
        assert_eq!(
            transparency_toggle_warning(software.clone(), false, false),
            None
        );
        // A saved "Don't show again" keeps it away.
        assert_eq!(transparency_toggle_warning(software, true, true), None);
    }
}

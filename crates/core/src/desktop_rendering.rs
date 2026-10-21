//! Testable Linux desktop rendering policy. Selection does not mutate the
//! environment; the native entry point applies it before GTK initializes.
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Gpu,
    SharedMemory,
    Software,
    X11,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::SharedMemory => "shared-memory",
            Self::Software => "software",
            Self::X11 => "x11",
        }
    }
    pub fn environment(self) -> Vec<(&'static str, &'static str)> {
        let mut env = vec![
            (
                "WEBKIT_DISABLE_DMABUF_RENDERER",
                if self == Self::Software { "1" } else { "0" },
            ),
            (
                "WEBKIT_DMABUF_RENDERER_FORCE_SHM",
                if self == Self::SharedMemory { "1" } else { "0" },
            ),
        ];
        if self == Self::X11 {
            env.push(("GDK_BACKEND", "x11"));
        }
        env
    }
}

fn enabled(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

pub fn select(
    requested: Option<&str>,
    disable_dmabuf: Option<&str>,
    force_shm: Option<&str>,
    nvidia_wayland: bool,
) -> Result<Mode> {
    match requested.filter(|s| !s.is_empty()) {
        Some("gpu") => return Ok(Mode::Gpu),
        Some("shared-memory") => return Ok(Mode::SharedMemory),
        Some("software") => return Ok(Mode::Software),
        Some("x11") => return Ok(Mode::X11),
        Some("auto") | None => {}
        Some(value) => {
            let message = format!(
                "Unknown MARQUEE_RENDERER={value}. Use auto, gpu, shared-memory, software, or x11."
            );
            return Err(AppError::new("RENDER_MODE", message));
        }
    }
    // Explicit Marquee modes take precedence; otherwise keep deliberate legacy overrides.
    if requested.is_none_or(str::is_empty) {
        if enabled(disable_dmabuf) {
            return Ok(Mode::Software);
        }
        if enabled(force_shm) {
            return Ok(Mode::SharedMemory);
        }
        if disable_dmabuf == Some("0") {
            return Ok(Mode::Gpu);
        }
    }
    Ok(if nvidia_wayland {
        Mode::SharedMemory
    } else {
        Mode::Gpu
    })
}

pub fn nvidia_wayland() -> bool {
    let backend = std::env::var("GDK_BACKEND").unwrap_or_default();
    let wayland = backend != "x11"
        && (backend == "wayland"
            || std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty())
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland"));
    wayland && std::path::Path::new("/sys/module/nvidia").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_preserves_acceleration_and_selects_transport_for_the_session() {
        assert_eq!(select(None, None, None, true).unwrap(), Mode::SharedMemory);
        assert_eq!(select(None, None, None, false).unwrap(), Mode::Gpu);
        for mode in [Mode::Gpu, Mode::SharedMemory, Mode::X11] {
            assert!(mode
                .environment()
                .contains(&("WEBKIT_DISABLE_DMABUF_RENDERER", "0")));
        }
    }
    #[test]
    fn explicit_modes_override_legacy_flags_and_invalid_modes_fail() {
        assert_eq!(
            select(Some("shared-memory"), Some("1"), None, false).unwrap(),
            Mode::SharedMemory
        );
        assert_eq!(
            select(Some("auto"), Some("1"), None, true).unwrap(),
            Mode::SharedMemory
        );
        assert_eq!(
            select(Some("gpu"), Some("1"), Some("1"), true).unwrap(),
            Mode::Gpu
        );
        assert!(select(Some("gup"), None, None, true).is_err());
        assert!(Mode::X11.environment().contains(&("GDK_BACKEND", "x11")));
    }
    #[test]
    fn deliberate_legacy_overrides_are_preserved() {
        assert_eq!(
            select(None, Some("1"), None, false).unwrap(),
            Mode::Software
        );
        assert_eq!(select(None, Some("0"), None, true).unwrap(), Mode::Gpu);
        assert_eq!(
            select(None, None, Some("1"), false).unwrap(),
            Mode::SharedMemory
        );
        assert!(Mode::Software
            .environment()
            .contains(&("WEBKIT_DISABLE_DMABUF_RENDERER", "1")));
    }
}

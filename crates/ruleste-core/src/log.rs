//! Leveled logging for the core crate.
//!
//! The original game routes everything through the `Logger` in `Celeste.cs`
//! (`Log.Info` / `Log.Warn` / `Log.Error`, with a `Log.LogLevel` gate and a
//! `Console`/`C:` log file). This module is the equivalent gate for Ruleste:
//! a process-global level that filters what the core and the client print, so
//! a headless test run can stay quiet while an interactive run stays verbose.
//!
//! Levels, from most to least verbose:
//!
//! | Level | Aliases | Typical content |
//! |-------|---------|-----------------|
//! | `trace` | `t` | Per-frame simulation steps (position deltas, collision probes) |
//! | `debug` | `d` | Asset counts, plugin load/skip decisions, state transitions |
//! | `info`  | `i` | Startup banners, level load, room switches |
//! | `warn`  | `w` | Recoverable problems (missing plugin, dropped sound) |
//! | `error` | `e` | Failures the caller must know about |
//! | `off`   | — | Silent (used by headless tests) |
//!
//! The level comes from, in order of precedence:
//!
//! 1. an explicit [`set_level`] call (what the `--log-level` CLI flag does),
//! 2. the `RULESTE_LOG_LEVEL` environment variable,
//! 3. the builder default ([`Level::Info`]).
//!
//! ```
//! ruleste_core::log::set_level_from_env();
//! ruleste_core::log_debug!("atlas: {} pages", 3);
//! ruleste_core::log_error!("failed to load {}", "Sprites.xml");
//! ```

use std::sync::atomic::{AtomicU8, Ordering};

/// Severity of a log record, ordered from most to least verbose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Per-frame simulation detail. Off by default; very noisy.
    Trace = 0,
    /// Asset counts, plugin decisions, recoverable state detail.
    Debug = 1,
    /// Startup banners and normal progress. The default.
    Info = 2,
    /// Something went wrong but the game keeps running.
    Warn = 3,
    /// A failure the caller needs to see.
    Error = 4,
    /// Emit nothing at all.
    Off = 5,
}

impl Level {
    /// Parses a level name, accepting the single-letter aliases.
    ///
    /// ```
    /// use ruleste_core::log::Level;
    /// assert_eq!(Level::parse("warn"), Some(Level::Warn));
    /// assert_eq!(Level::parse(" D "), Some(Level::Debug));
    /// assert_eq!(Level::parse("loud"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "trace" | "t" => Some(Level::Trace),
            "debug" | "d" => Some(Level::Debug),
            "info" | "i" => Some(Level::Info),
            "warn" | "warning" | "w" => Some(Level::Warn),
            "error" | "e" => Some(Level::Error),
            "off" | "none" | "silent" | "quiet" => Some(Level::Off),
            _ => None,
        }
    }

    /// The canonical lowercase name of this level.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
            Level::Off => "off",
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Current level. `Info` until something overrides it, so a plain `cargo run`
/// behaves like the original's default `Log.Info` output.
static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

/// Returns the active level.
#[must_use]
pub fn level() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Level::Trace,
        1 => Level::Debug,
        2 => Level::Info,
        3 => Level::Warn,
        4 => Level::Error,
        _ => Level::Off,
    }
}

/// Sets the active level.
pub fn set_level(l: Level) {
    LEVEL.store(l as u8, Ordering::Relaxed);
}

/// Applies `RULESTE_LOG_LEVEL` if it is set to a known level.
///
/// Returns the resulting level so callers can report the choice. An unset or
/// unparseable variable leaves the current level untouched, which keeps a typo
/// from silently muting the game.
pub fn set_level_from_env() -> Level {
    if let Some(raw) = std::env::var_os("RULESTE_LOG_LEVEL") {
        let raw = raw.to_string_lossy().into_owned();
        match Level::parse(&raw) {
            Some(l) => {
                set_level(l);
                l
            }
            None => level(),
        }
    } else {
        level()
    }
}

/// True when a record at `lvl` would be emitted.
#[must_use]
pub fn enabled(lvl: Level) -> bool {
    lvl >= level()
}

/// Writes one record to stderr, tagged with the level.
///
/// stderr rather than stdout so `--log-level` output can be separated from a
/// tool's machine-readable stdout (the frame-dump and headless harness paths
/// both read stdout).
pub fn log(lvl: Level, args: std::fmt::Arguments<'_>) {
    if !enabled(lvl) {
        return;
    }
    let tag = match lvl {
        Level::Trace => "trace",
        Level::Debug => "debug",
        Level::Info => "info ",
        Level::Warn => "warn ",
        Level::Error => "error",
        Level::Off => return,
    };
    eprintln!("[{tag}] {args}");
}

/// Logs at [`Level::Trace`].
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Trace, format_args!($($arg)*)) };
}

/// Logs at [`Level::Debug`].
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Debug, format_args!($($arg)*)) };
}

/// Logs at [`Level::Info`].
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Info, format_args!($($arg)*)) };
}

/// Logs at [`Level::Warn`].
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Warn, format_args!($($arg)*)) };
}

/// Logs at [`Level::Error`].
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::log($crate::log::Level::Error, format_args!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The level global is process-wide, so these tests must not run in
    /// parallel with each other. A single test function keeps that guarantee
    /// without pulling in a dependency.
    #[test]
    fn level_parsing_round_trips_and_aliases() {
        for l in [
            Level::Trace,
            Level::Debug,
            Level::Info,
            Level::Warn,
            Level::Error,
            Level::Off,
        ] {
            assert_eq!(Level::parse(l.as_str()), Some(l));
        }
        assert_eq!(Level::parse("warning"), Some(Level::Warn));
        assert_eq!(Level::parse(" d "), Some(Level::Debug));
        assert_eq!(Level::parse("nonsense"), None);
        assert_eq!(Level::parse(""), None);
    }

    #[test]
    fn level_gate_filters_by_severity() {
        let saved = level();
        set_level(Level::Warn);
        assert!(enabled(Level::Warn));
        assert!(enabled(Level::Error));
        assert!(!enabled(Level::Info));
        assert!(!enabled(Level::Trace));
        set_level(Level::Off);
        assert!(!enabled(Level::Error), "off silences even errors");
        set_level(Level::Trace);
        assert!(enabled(Level::Trace));
        set_level(saved);
    }
}

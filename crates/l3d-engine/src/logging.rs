//! Log messages to the terminal, or on Android to the system log: `info`
//! and up, but only warnings and errors from wgpu and naga, which are
//! chatty.

use std::sync::Once;

struct Logger;

static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        let quiet = ["wgpu", "naga"].iter().any(|t| m.target().starts_with(t));
        m.level()
            <= if quiet {
                log::Level::Warn
            } else {
                log::Level::Info
            }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        write(record);
    }

    fn flush(&self) {}
}

#[cfg(not(target_os = "android"))]
fn write(record: &log::Record) {
    eprintln!(
        "{:>5} {}: {}",
        record.level(),
        record.target(),
        record.args()
    );
}

#[cfg(target_os = "android")]
fn write(record: &log::Record) {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    let prio = match record.level() {
        log::Level::Error => 6,
        log::Level::Warn => 5,
        log::Level::Info => 4,
        log::Level::Debug => 3,
        log::Level::Trace => 2,
    };
    let tag = CString::new(record.target().replace('\0', "")).unwrap_or_default();
    let text = CString::new(record.args().to_string().replace('\0', "")).unwrap_or_default();
    // SAFETY: both are NUL-terminated strings that outlive the call.
    unsafe {
        __android_log_write(prio, tag.as_ptr(), text.as_ptr());
    }
}

/// Starts logging (once).
pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
    });
}

/// Logs a warning the first time this line is reached.
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)+) => {{
        static ONCE: ::std::sync::Once = ::std::sync::Once::new();
        ONCE.call_once(|| $crate::log::warn!($($arg)+));
    }};
}

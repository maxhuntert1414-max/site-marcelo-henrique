#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::{boost_thread_priority, main, protect, unprotect};

#[cfg(not(windows))]
mod headless;
#[cfg(not(windows))]
pub use self::headless::{boost_thread_priority, main, protect, unprotect};

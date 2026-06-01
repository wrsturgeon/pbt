//! Catch panics without letting their intermediate diagnostics reach stderr.

use {
    alloc::string::String,
    core::{any::Any, cell::Cell, panic::UnwindSafe},
    std::{panic, sync::Once, thread_local},
};

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Install the delegating panic hook at most once.
static INSTALL_HOOK: Once = Once::new();

/// A scoped guard that suppresses this thread's panic hook output.
struct Quiet {
    /// The previous suppression flag, restored when this guard is dropped.
    restore: bool,
}

impl Quiet {
    /// Suppress panic hook output until the returned guard is dropped.
    #[inline]
    fn new() -> Self {
        install_hook();
        Self {
            restore: QUIET.with(|quiet| quiet.replace(true)),
        }
    }
}

impl Drop for Quiet {
    #[inline]
    fn drop(&mut self) {
        QUIET.with(|quiet| quiet.set(self.restore));
    }
}

/// Install a panic hook that can delegate or suppress on each thread.
#[inline]
fn install_hook() {
    INSTALL_HOOK.call_once(|| {
        let hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if QUIET.with(Cell::get) {
                return;
            }
            let () = hook(info);
        }));
    });
}

/// Extract the ordinary Rust panic message, if the payload uses one.
#[inline]
fn message(panic: &(dyn Any + Send)) -> Option<String> {
    panic
        .downcast_ref::<&'static str>()
        .map(|s| String::from(*s))
        .or_else(|| panic.downcast_ref::<String>().cloned())
}

/// Run a PBT candidate, suppressing its panic hook and recovering its message.
///
/// This is used by `#[pbt]` while searching and shrinking so that only the
/// final, minimized panic is printed by the test runner.
///
/// # Errors
///
/// Returns the recovered panic message if `f` panics.
#[inline]
pub fn catch<F>(f: F) -> Result<(), Option<String>>
where
    F: FnOnce() + UnwindSafe,
{
    let _quiet = Quiet::new();
    panic::catch_unwind(f).map_err(|panic| message(&*panic))
}

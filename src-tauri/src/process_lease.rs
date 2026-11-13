//! Session lock shared by the UI, MCP and CLI processes.
//!
//! The lock file stays in place: deleting or replacing it would create
//! another inode and allow two processes to believe they hold exclusivity.

use std::fs::{File, OpenOptions};
use std::io;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::path::Path;

pub(crate) const LOCK_FILE: &str = "humcase.session.lock";

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Shared,
    Exclusive,
    Unlocked,
}

/// Outcome of an operation attempted under an exclusive lock.
///
/// Lets the caller tell apart several situations that a plain
/// `io::Result<T>` would conflate:
/// - an operation that was **not** run (exclusive lock unavailable);
/// - an operation that ran, whose value is known and the shared lock was retaken;
/// - critical point: an operation that **succeeded and carries its value** but whose
///   retaking of the shared lock failed. Without this distinction, the caller
///   cannot know whether a side effect (e.g. a committed restore) took
///   place and risks reopening writes wrongly.
#[derive(Debug)]
pub enum ExclusiveOutcome<T> {
    /// The session did not hold the shared lock: invalid internal state,
    /// operation not run.
    NotShared(io::Error),
    /// (a) The exclusive lock was not available: the operation was NOT
    /// run.
    LockUnavailable(io::Error),
    /// (b) The operation ran and the shared lock was successfully retaken.
    /// `T` carries the operation's own value (possibly an `Err` `Result`):
    /// the caller derives its report from it.
    Completed(T),
    /// (c) The operation ran and carries its value `value` (for example an
    /// `Ok(())` meaning it committed), but retaking the shared lock
    /// failed. The value must not be lost.
    ///
    /// The name refers to retaking the **lock** ("Restore" as in *restore the
    /// shared lease*), not to a data restore; the value
    /// `T` alone tells what the operation actually published.
    LockRestoreFailed { value: T, error: io::Error },
}

/// Holds a shared lock for the whole life of a Humcase process.
/// Closing the file also releases the lock if the process stops.
#[derive(Debug)]
pub struct ProcessLease {
    file: File,
    mode: Mode,
}

impl ProcessLease {
    /// Refuses to start immediately if a restore holds exclusivity.
    /// To be called before any opening or migration of the SQLite database.
    pub fn acquire_shared(app_dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(app_dir)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(app_dir.join(LOCK_FILE))?;
        file.try_lock_shared()?;
        Ok(Self {
            file,
            mode: Mode::Shared,
        })
    }

    /// Gives up our shared read lock, tries exclusivity without waiting, then retakes
    /// the shared lock even if the operation returns an error or panics.
    /// Closing the SQLite connections and the internal mutation barrier
    /// are the caller's responsibility.
    ///
    /// See [`ExclusiveOutcome`]: outcomes are kept distinct so that a
    /// caller never has to confuse "the operation did not run" with
    /// "the operation succeeded but the shared lock was not retaken".
    pub fn with_exclusive<T>(&mut self, operation: impl FnOnce() -> T) -> ExclusiveOutcome<T> {
        self.with_exclusive_using(operation, Self::restore_shared)
    }

    /// Core of [`Self::with_exclusive`]. Retaking the lock is injectable so
    /// that a deterministic test can simulate a failure **after** a
    /// successful operation, without depending on the file system.
    ///
    /// Testability limit: this injection does not reproduce the real residual
    /// state of the lease. A real `restore_shared` failure (failed `unlock()` then
    /// failed `lock_shared()`) cannot be injected: it depends on the
    /// file system and leaves the process in an intermediate mode
    /// (`Unlocked`/`Exclusive`) that the injection does not recreate. Tests that
    /// verify the mode after a real-lock scenario use
    /// `current_mode` (defined under `cfg(test)`); the injection only covers
    /// the **value** passed to the caller.
    fn with_exclusive_using<T, R>(
        &mut self,
        operation: impl FnOnce() -> T,
        restore: R,
    ) -> ExclusiveOutcome<T>
    where
        R: FnOnce(&mut Self) -> io::Result<()>,
    {
        if self.mode != Mode::Shared {
            return ExclusiveOutcome::NotShared(io::Error::other(crate::tr!("app.leaseNotHeld")));
        }

        if let Err(error) = self.file.unlock() {
            return ExclusiveOutcome::LockUnavailable(error);
        }
        self.mode = Mode::Unlocked;

        if let Err(error) = self.file.try_lock() {
            // Exclusivity is unavailable: the operation does not run. We still try
            // to retake the shared lock; if that fails too, the retake error
            // is the most useful one to report.
            if let Err(restore_error) = restore(self) {
                return ExclusiveOutcome::LockUnavailable(restore_error);
            }
            return ExclusiveOutcome::LockUnavailable(error.into());
        }
        self.mode = Mode::Exclusive;

        let result = catch_unwind(AssertUnwindSafe(operation));
        let restore_result = restore(self);

        match result {
            Ok(value) => match restore_result {
                Ok(()) => ExclusiveOutcome::Completed(value),
                Err(error) => ExclusiveOutcome::LockRestoreFailed { value, error },
            },
            Err(payload) => {
                // Do not let a panic mask a failure to restore the
                // lock: two simultaneous errors require stopping the process.
                restore_result.expect("unable to retake the session lock after a panic");
                resume_unwind(payload)
            }
        }
    }

    fn restore_shared(&mut self) -> io::Result<()> {
        if self.mode == Mode::Exclusive {
            self.file.unlock()?;
            self.mode = Mode::Unlocked;
        }
        self.file.lock_shared()?;
        self.mode = Mode::Shared;
        Ok(())
    }

    /// **Test** accessor: allows asserting the mode actually reached after
    /// a real-lock scenario (the injection of [`Self::with_exclusive_using`]
    /// only covers the retake, not the failure of `unlock()`).
    #[cfg(test)]
    pub(crate) fn current_mode(&self) -> &'static str {
        match self.mode {
            Mode::Shared => "shared",
            Mode::Exclusive => "exclusive",
            Mode::Unlocked => "unlocked",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExclusiveOutcome, ProcessLease};

    #[test]
    fn exclusive_requires_no_other_process_lease() {
        let directory = tempfile::tempdir().unwrap();
        let mut first = ProcessLease::acquire_shared(directory.path()).unwrap();
        let second = ProcessLease::acquire_shared(directory.path()).unwrap();

        assert!(matches!(
            first.with_exclusive(|| ()),
            ExclusiveOutcome::LockUnavailable(_)
        ));
        drop(second);
        match first.with_exclusive(|| {
            assert!(ProcessLease::acquire_shared(directory.path()).is_err());
            42
        }) {
            ExclusiveOutcome::Completed(value) => assert_eq!(value, 42),
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _third = ProcessLease::acquire_shared(directory.path()).unwrap();
    }

    #[test]
    fn shared_lock_returns_after_operation_error_and_panic() {
        let directory = tempfile::tempdir().unwrap();
        let mut first = ProcessLease::acquire_shared(directory.path()).unwrap();

        let operation: std::io::Result<()> =
            match first.with_exclusive(|| Err(std::io::Error::other("failure"))) {
                ExclusiveOutcome::Completed(value) => value,
                other => panic!("unexpected outcome: {other:?}"),
            };
        assert!(operation.is_err());
        let second = ProcessLease::acquire_shared(directory.path()).unwrap();
        drop(second);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = first.with_exclusive(|| panic!("operation failed"));
        }));
        assert!(panic.is_err());
        let _third = ProcessLease::acquire_shared(directory.path()).unwrap();
    }

    /// Deterministic injection: a **successful** operation whose lock retake
    /// fails must produce a distinct outcome that **keeps the value** of the
    /// operation. This is what lets the caller know the side effect
    /// did happen (e.g. committed restore) despite the lock failure.
    #[test]
    fn restore_failure_after_success_is_distinct_and_keeps_value() {
        let directory = tempfile::tempdir().unwrap();
        let mut first = ProcessLease::acquire_shared(directory.path()).unwrap();

        let outcome = first.with_exclusive_using(
            || Ok::<u32, String>(7),
            |_lease| Err(std::io::Error::other("retake failed")),
        );

        match outcome {
            ExclusiveOutcome::LockRestoreFailed { value, error } => {
                assert_eq!(value, Ok(7), "the operation's value must be kept");
                assert_eq!(error.kind(), std::io::ErrorKind::Other);
            }
            other => panic!("expected retake failure, got: {other:?}"),
        }
        // The injection did not touch the real mode: the lease stays `exclusive`, concrete
        // proof that `with_exclusive_using` does not reproduce the residual state of a real
        // `restore_shared` failure (documented limit).
        assert_eq!(first.current_mode(), "exclusive");
    }

    /// A **real-lock** scenario leaves the lease in shared read mode after
    /// a successful operation as after a panic: this is what the injection
    /// cannot observe, hence the [`ProcessLease::current_mode`] accessor.
    #[test]
    fn real_lock_scenarios_restore_shared_mode() {
        let directory = tempfile::tempdir().unwrap();
        let mut lease = ProcessLease::acquire_shared(directory.path()).unwrap();
        assert_eq!(lease.current_mode(), "shared");

        let outcome = lease.with_exclusive(|| 1u32);
        assert!(matches!(outcome, ExclusiveOutcome::Completed(1)));
        assert_eq!(lease.current_mode(), "shared");

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = lease.with_exclusive(|| panic!("operation failed"));
        }));
        assert!(panic.is_err());
        assert_eq!(lease.current_mode(), "shared");
    }
}

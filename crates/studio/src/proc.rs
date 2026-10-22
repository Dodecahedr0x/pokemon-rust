//! Child processes the studio starts (the ROM build, the explorer) in their own process group
//! on unix, so that stopping one also kills what it started (the inner cargo, rustc), not only
//! the top-level `cargo`.

use std::process::{Child, Command};

#[cfg(unix)]
unsafe extern "C" {
    /// libc's `kill(2)`.
    fn kill(pid: i32, sig: i32) -> i32;
}
#[cfg(unix)]
const SIGKILL: i32 = 9;

/// Makes `cmd` start a new process group (unix; a no-op elsewhere). The group's id is the
/// child's pid.
pub fn new_group(cmd: &mut Command) {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(cmd, 0);
    #[cfg(not(unix))]
    let _ = cmd;
}

/// Kills `child`'s process group (unix, see [`new_group`]) and `child` itself. The caller reaps
/// the child (`wait`) right after: until then the group id cannot be reused.
pub fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(pgid) = i32::try_from(child.id()) {
        // SAFETY: a plain syscall. `-pgid` names the group `new_group` created; its id is the
        // child's pid, not reused while the child is unreaped or while the group has members.
        unsafe {
            kill(-pgid, SIGKILL);
        }
    }
    let _ = child.kill();
}

/// Whether process `pid` exists (signal 0).
#[cfg(unix)]
pub fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { kill(pid, 0) == 0 }
}

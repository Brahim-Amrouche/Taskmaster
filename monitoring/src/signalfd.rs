use std::ffi::{c_int, c_ulong, c_void};
use std::os::fd::RawFd;

/// Config-reload signal. The only signal we deliver via signalfd.
pub const SIGHUP: c_int = 1;
pub const SIG_BLOCK: c_int = 0;

// --- signalfd consttants ---
pub const SFD_CLOEXEC: c_int =  0o2000000;
pub const SFD_NONBLOCK: c_int = 0o4000;

pub const SIGINFO_SIZE: usize = 128; // sizeof(struct signalfd_siginfo) on x86_64

/// `sigset_t` layout: glibc defines it as 1024 bits laid out as
/// `unsigned long[16]` on 64-bit Linux.
#[repr(C)]
pub struct SigSet {
    val: [c_ulong; 16]
}

impl SigSet {
    pub fn empty() -> Self {
        Self { val: [0; 16] }
    }
}

#[link(name = "c")]
unsafe extern "C" {
    fn sigemptyset(set: *mut SigSet) -> c_int;
    fn sigaddset(set: *mut SigSet, signum: c_int) -> c_int;
    fn sigprocmask(how: c_int, set: *const SigSet, oldset: *mut SigSet) -> c_int;
    fn signalfd(fd: c_int, mask: *const SigSet, flags: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
}

/// Zero every bit in `set`. Wrapper over `sigemptyset(3)`.
pub fn emptyset(set: &mut SigSet) -> std::io::Result<()> {
    let rc = unsafe {
        sigemptyset(set as *mut SigSet)
    };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Add one signal to the mask. Wrapper over `sigaddset(3)`.
pub fn addset(set: &mut SigSet, signum: c_int) -> std::io::Result<()> {
    let rc = unsafe {
        sigaddset(set as *mut SigSet, signum)
    };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Apply `set` to the process signal mask, using `how` to pick the
/// semantics (`SIG_BLOCK` for a supervisor's needs).
/// Wrapper over `sigprocmask(2)`; passes `null` for `oldset`.
pub fn procmask(how: c_int, set: &SigSet) -> std::io::Result<()> {
    // SAFETY: `set` is a live, readable SigSet; null `oldset` is valid.
    let rc = unsafe {
        sigprocmask(how, set, std::ptr::null_mut())
    };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub fn block_sighup() -> std::io::Result<()> {
    let mut set = SigSet::empty();
    emptyset(&mut set)?;
    addset(&mut set, SIGHUP)?;
    procmask(SIG_BLOCK, &set)
}

pub struct SignalFd {
    fd: RawFd,
}

impl SignalFd {
    /// Open a signalfd that delivers SIGHUP. Blocks SIGHUP process-wide
    /// first (otherwise the default disposition terminates the process
    /// before the fd can report anything).
    pub fn open_sighup() -> std::io::Result<Self> {
        block_sighup()?;
        
        let mut mask = SigSet::empty();
        emptyset(&mut mask)?;
        addset(&mut mask, SIGHUP)?;

        // SAFETY: mask is live and well-formed; fd=-1 asks the kernel to
        // allocate a fresh signalfd.
        let fd = unsafe {
            signalfd(-1, &mask, SFD_CLOEXEC | SFD_NONBLOCK)
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.fd
    }

    /// Drain all pending SIGHUPs. Returns the count. Caller should reload
    /// once if count > 0, regardless of the exact number.
    pub fn drain(&self) -> std::io::Result<usize> {
        let mut count: usize = 0;
        let mut buf = [0u8; SIGINFO_SIZE];
        loop {
            let n = unsafe {
                read(self.fd, buf.as_mut_ptr() as *mut c_void, buf.len())
            };
            match n {
                -1 => {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::WouldBlock {
                        return Ok(count);
                    }
                    return Err(err);

                }
                n if n as usize == SIGINFO_SIZE => {
                    count += 1;
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "signalfd read returned unexpected number of bytes"
                    ))
                }

            }

        }
    }
}

impl Drop for SignalFd {
    fn drop(&mut self) {
        // SAFETY: we own this fd; close errors are not actionable in drop.
        unsafe {
            close(self.fd);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_sighup_mask() {
        let mut set = SigSet::empty();
        emptyset(&mut set).expect("sigemptyset");
        addset(&mut set, SIGHUP).expect("sigaddset SIGHUP");
    }

    #[test]
    fn block_sighup_round_trip() {
        // Idempotent on success — calling it twice is also legal.
        block_sighup().expect("block SIGHUP first time");
        block_sighup().expect("block SIGHUP second time");
        // After this test, SIGHUP stays blocked for the rest of the test
        // process. That's fine: step-3 will create the signalfd that will
        // then deliver any later SIGHUP via the fd.
    }

    #[test]
    fn sighup_signalfd_opens() {
        let sfd = SignalFd::open_sighup().expect("open signalfd");
        assert!(sfd.as_raw_fd() >= 0);
    }

    #[test]
    fn sighup_arrives_and_drains() {
        #[link(name = "c")]
        unsafe extern "C" { fn raise(sig: c_int) -> c_int; }

        let sfd = SignalFd::open_sighup().expect("open");
        assert_eq!(sfd.drain().expect("drain empty"), 0);

        // SAFETY: raise() is thread-directed; the signal lands on this thread,
        // which has SIGHUP blocked and the signalfd open.
        assert_eq!(unsafe { raise(SIGHUP) }, 0);
        assert_eq!(unsafe { raise(SIGHUP) }, 0);  // second one coalesces

        let n = sfd.drain().expect("drain");
        assert!(n >= 1, "expected at least one SIGHUP delivered");
        assert_eq!(sfd.drain().expect("drain empty again"), 0);
    }
}


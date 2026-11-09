#![cfg(target_os = "linux")]

pub mod signalfd;

use std::ffi::{c_int, c_long, c_ulong, c_void, c_short};
use std::io;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Fd(pub c_int);

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Pid(pub c_int);

pub type SizeT = c_ulong;
pub type SSizeT = c_long;

// --- syscall number (x86_64 Linux) ---
pub const SYS_PERF_EVENT_OPEN: c_long = 298;

// --- type + config ---
pub const PERF_TYPE_SOFTWARE: u32 = 1;

pub const PERF_COUNT_SW_CPU_CLOCK: u64 = 0;
pub const PERF_COUNT_SW_TASK_CLOCK: u64 = 1;
pub const PERF_COUNT_SW_PAGE_FAULTS: u64 = 2;
pub const PERF_COUNT_SW_CONTEXT_SWITCHES: u64 = 3;

// --- a software "counter" that doesn't count anything.
//     Used as a side-band carrier for FORK / EXIT / COMM records. ---
pub const PERF_COUNT_SW_DUMMY: u64 = 9;

// --- side-band record types written to the ring buffer by the kernel ---
pub const PERF_RECORD_LOST: u32 = 2;
pub const PERF_RECORD_COMM: u32 = 3;
pub const PERF_RECORD_EXIT: u32 = 4;
pub const PERF_RECORD_FORK: u32 = 7;

// --- bit positions inside the `flags` u64 of PerfEventAttr ---
pub const ATTR_DISABLED: u64 = 1 << 0;
pub const ATTR_INHERIT: u64 = 1 << 1;
pub const ATTR_EXCLUDE_KERNEL: u64 = 1 << 5;
pub const ATTR_COMM: u64 = 1 << 9;
pub const ATTR_ENABLE_ON_EXEC: u64 = 1 << 12;
pub const ATTR_TASK: u64 = 1 << 13;
pub const ATTR_WATERMARK: u64 = 1 << 14;

// --- flag for the 5th syscall arg ---
pub const PERF_FLAG_FD_CLOEXEC: c_ulong = 1 << 3;


// --- system conf ---
pub const _SC_PAGESIZE: c_int = 30;

// --- mmap prot + flags ---
pub const PROT_READ: c_int = 0x1;
pub const PROT_WRITE: c_int = 0x2;
pub const MAP_SHARED: c_int = 0x1;

/// `MAP_FAILED` sentinel: `((void *) -1)` in C.
#[inline]
fn map_failed() -> *mut c_void {
    !0usize as *mut c_void
}

// --- poll flags ---
pub const POLLIN: c_short = 0x001;
pub const POLLERR: c_short = 0x008;
pub const POLLHUP: c_short = 0x010;
pub const POLLNVAL: c_short = 0x020;

// --- wait4 options ---
pub const WNOHANG: c_int = 1;

// --- status word helpers ---
#[inline] pub fn w_if_exited(status: c_int) -> bool { (status & 0x7f) == 0 }
#[inline] pub fn w_exit_status(status: c_int) -> c_int { (status >> 8) & 0xff }
#[inline] pub fn w_if_signaled(status: c_int) -> bool {
    let low = status & 0x7f;
    low != 0 && low != 0x7f
}
#[inline] pub fn w_term_sig(status: c_int) -> c_int { status & 0x7f }



// --- attribute struct the kernel reads ---
#[repr(C)]
#[derive(Default)]
pub struct PerfEventAttr {
    pub type_: u32,
    pub size: u32,
    pub config: u64,
    pub sample: u64,
    pub sample_type: u64,
    pub read_format: u64,
    pub flags: u64,
    pub wakeup: u32,
    pub bp_type: u32,
    pub bp_addr: u64,
    pub bp_len: u64,
    pub branch_sample_type: u64,
    pub sample_regs_user: u64,
    pub sample_stack_user: u32,
    pub clockid: i32,
    pub sample_regs_intr: u64,
    pub aux_watermark: u32,
    pub sample_max_stack: u16,
    pub __reserved_2: u16,
    pub aux_sample_size: u32,
    pub __reserved_3: u32,
    pub sig_data: u64,
    pub config3: u64,
}

#[repr(C)]
pub struct PerfEventMmapPage {
    /// Opaque: version, lock, time counters, capabilities, etc.
    /// 1024 bytes of fields we don't need; the kernel fills them.
    pub reserved: [u8; 1024],

    /// Written by the kernel when it appends a record. Read with
    /// an **acquire** load.
    pub data_head: u64,

    /// Written by us after we consume records. Written with a
    /// **release** store.
    pub data_tail: u64,

    /// Offset into the mmap region where the data buffer starts.
    /// Usually equals `page_size`.
    pub data_offset: u64,

    /// Size of the data buffer in bytes (power of two × page_size).
    pub data_size: u64,
}

const _: () = assert!(
    std::mem::offset_of!(PerfEventMmapPage, data_head) == 1024,
    "data_head must sit at offset 1024 to match the kernel ABI"
);

// --- raw C symbols from libc ---
#[link(name = "c")]
unsafe extern "C" {
    fn syscall(num: c_long, ...) -> c_long;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn close(fd: c_int) -> c_int;
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, offset: isize) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn sysconf(name: c_int) -> c_long;
    fn poll(fds: *mut PollFd, nfds: c_ulong, timeout: c_int) -> c_int;
    fn wait4(pid: c_int, wstatus: *mut c_int, options: c_int, rusage: *mut c_void) -> c_int;
}

/// safe wraper around 'poll' syscall
pub fn poll_fds(fds: &mut [PollFd], timeout_ms: c_int) -> io::Result<usize> {
    // SAFETY: slice gives us a valid ptr + length pair.
    let rc = unsafe {
        poll(fds.as_mut_ptr(), fds.len() as c_ulong, timeout_ms)
    };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc as usize)
    }
}

pub fn wait4pid(pid: c_int, options: c_int) -> io::Result<c_int> {
    let mut status: c_int = 0;
    // SAFETY: &mut status is a valid pointer to a c_int.
    let rc = unsafe {
        wait4(pid, &mut status as *mut c_int, options, std::ptr::null_mut())
    };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(status)
    }
}


/// Thin wrapper over the `perf_event_open(2)` syscall.
///
/// Returns the new counter fd (`>= 0`) on success, or `-1` on failure
/// with `errno` set — mirror of the raw libc `syscall(2)` convention.
///
/// # Safety
///
/// The caller must guarantee that `attr.size` equals
/// `std::mem::size_of::<PerfEventAttr>()`. The kernel reads exactly that
/// many bytes starting at `attr` and relies on the value being accurate;
/// a wrong size produces either silent truncation (`attr.size <` real
/// struct size) or `E2BIG` from the kernel (`attr.size >` what the
/// kernel knows). Rust's reference guarantees the pointer itself is
/// valid for reads, but the ABI contract on `size` is on the caller.
pub(crate) unsafe fn perf_event_open_raw(
    attr: &PerfEventAttr,
    pid: Pid,
    cpu: c_int,
    group_fd: Fd,
    flags: c_ulong,
) -> c_long {
    unsafe {
        syscall(
            SYS_PERF_EVENT_OPEN,
            attr as *const PerfEventAttr,
            pid.0,
            cpu,
            group_fd.0,
            flags,
        )
    }
}

pub struct PerfCounter {
    fd: Fd,
}

impl PerfCounter {
    /// Open a per-process counter for the configuration in `attr`.
    ///
    /// `pid = 0` means the calling process; `cpu = -1` means any CPU.
    /// For a software counter attached to our own task, pass `(0, -1)`.
    ///
    /// The `size` field of `attr` is overwritten with the correct value
    /// before the syscall, so callers never have to set it themselves.
    pub fn open(attr: &mut PerfEventAttr, pid: Pid, cpu: c_int) -> io::Result<Self> {
        attr.size = std::mem::size_of::<PerfEventAttr>() as u32;

        // SAFETY: `attr.size` has just been set to the correct struct size,
        // satisfying the only precondition of `perf_event_open_raw`.
        let rc = unsafe {
             perf_event_open_raw(attr, pid, cpu, Fd(-1), PERF_FLAG_FD_CLOEXEC) 
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd: Fd(rc as c_int) })
    }

    /// Open a counter whose only purpose is to deliver side-band records
    /// (fork, exit, comm) into its ring buffer. The counter itself is a
    /// `PERF_COUNT_SW_DUMMY` — it never increments; we only use its fd
    /// as an event channel.
    ///
    /// Pass the target process's `pid`. With `ATTR_INHERIT`, records are
    /// also emitted for the target's descendants, which is what a
    /// supervisor wants: it sees a child's own forks and that child's exit.
    ///
    /// `cpu` is fixed at `-1` (any CPU).
    /// 
    /// ommit ATTR_INHERIT flag: kernal refuse mmap on per-task counter with
    ///  inherit flag. tTo track children we need to open one counter per CPU.
    pub fn open_task_tracker(pid: Pid) -> io::Result<Self> {
        let mut attr = PerfEventAttr::default();
        attr.type_  = PERF_TYPE_SOFTWARE;
        attr.config = PERF_COUNT_SW_DUMMY;
        attr.flags  = ATTR_TASK
                    | ATTR_COMM
                    | ATTR_EXCLUDE_KERNEL
                    | ATTR_WATERMARK; // ommit ATTR_INHERIT flag
        attr.wakeup = 1;        // wakeup_watermark = 1 byte → wake on every record
        attr.sample = 0;        // no sampling, just side-band records
        attr.sample_type = 0;   // -
        attr.read_format = 0;   // -

        Self::open(&mut attr, pid, -1)

    }

    pub fn open_tree_tracker_cpu(pid: Pid, cpu: c_int) -> io::Result<Self> {
        let mut attr = PerfEventAttr::default();
        attr.type_  = PERF_TYPE_SOFTWARE;
        attr.config = PERF_COUNT_SW_DUMMY;
        attr.flags  = ATTR_TASK
                    // | ATTR_COMM
                    | ATTR_EXCLUDE_KERNEL
                    | ATTR_WATERMARK
                    | ATTR_INHERIT; // inherit flag to track children
        attr.wakeup = 1;        // wakeup_watermark = 1 byte → wake on every record
        attr.sample = 0;        // no sampling, just side-band records
        attr.sample_type = 0;   // -
        attr.read_format = 0;   // -

        Self::open(&mut attr, pid, cpu)
    }

    /// Current counter value. Reads 8 native-endian bytes from the fd.
    pub fn read_count(&self) -> io::Result<u64> {
        let mut buff = [0u8; 8];
        // SAFETY: `self.fd` is a valid open descriptor owned by `self`;
        // `buf` is a live writable slice of exactly 8 bytes.
        let n = unsafe {
            read(self.fd.0, buff.as_mut_ptr() as *mut c_void, buff.len())
        };
        match n {
            -1 => Err(io::Error::last_os_error()),
            8 => Ok(u64::from_ne_bytes(buff)),
            _ => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("short perf read: expected 8 bytes, got {n}"),
            )),
        }
    }

    /// Access the raw fd, e.g. to pass it to `ioctl` for enable/disable.
    pub fn as_raw_fd(&self) -> RawFd {
        self.fd.0
    }

    /// Map a ring buffer onto this counter's fd.
    ///
    /// `data_pages_log2 = 4` gives you 16 data pages. Pick small for
    /// task tracking (we only need to buffer burstiness between poll
    /// wakeups), large for sampling.
    pub fn attach_ring(&self, data_pages_log2: u32) -> io::Result<PerfRing> {
        PerfRing::new(self, data_pages_log2)
    }
    
}

impl Drop for PerfCounter {
    fn drop(&mut self) {
        // SAFETY: `self.fd` was returned by a successful `perf_event_open`
        // and no other reference to it exists — we own it. `close` errors
        // are not actionable at drop time, so they are ignored.
        unsafe { close(self.fd.0) };
    }
}


pub struct PerfRing {
    pub base: *mut u8,      // start of mmap region
    pub total_len: usize,   // page_size + data_size
    pub page_size: usize,
    pub data_size: usize,
    pub fd: Fd,          // for poll(2) and drop
}

impl PerfRing {
    /// Map `2^data_pages_log2` data pages plus the kernel's header page.
    /// Must be a power of two; e.g. `4` → 16 pages ≈ 64 KiB on a 4K kernel.
    pub fn new(perf_counter: &PerfCounter, data_pages_log2: u32) -> io::Result<Self> {
        let page_size = unsafe { sysconf(_SC_PAGESIZE) } as usize;
        let data_size = (1usize << data_pages_log2) * page_size;
        let total_len = page_size + data_size;

        // SAFETY: MAP_SHARED mmap of a perf fd; the kernel will fill in
        // the header page and manage the data buffer. We own the mapping
        // and `munmap` it in `Drop`.
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                total_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                perf_counter.as_raw_fd(),
                0
            )
        };
        if ptr == map_failed() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            base: ptr as *mut u8,
            total_len,
            page_size,
            data_size,
            fd: perf_counter.fd 
        })
    }

    /// The kernel's header page. Read-only to us for `data_head`,
    /// write for `data_tail`.
    pub fn header(&self) -> &PerfEventMmapPage {
        // SAFETY: `base` is a live mmap region at least `page_size` long;
        // `PerfEventMmapPage` is `#[repr(C)]` and fits in the first page.
        unsafe { &*(self.base as *const PerfEventMmapPage) }
    }

    /// Pointer to the start of the data buffer where records are written.
    pub fn data_ptr(&self) -> *mut u8 {
        // Data starts one page in; data_offset from the kernel is also
        // page_size on every current kernel, but using our own constant
        // keeps us honest about where we mapped.
        // SAFETY: offset is in-bounds of the mmap.
        unsafe { self.base.add(self.page_size) }
    }

    pub fn data_size(&self) -> usize {
        self.data_size
    }

    /// Position where the kernel will write next.
    /// Acquire-ordered: pairs with the kernel's release-store on append.
    pub fn data_head(&self) -> u64 {
        let offset = std::mem::offset_of!(PerfEventMmapPage, data_head);
        // SAFETY: `self.base` is a live mmap of at least one page;
        // `data_head` is at a fixed offset inside the first page.
        // `AtomicU64` has the same layout as `u64`.
        unsafe {
            let ptr = self.base.add(offset) as *const AtomicU64;
            (*ptr).load(Ordering::Acquire)
        }
    }

    /// Position up to which we have consumed records.
    pub fn data_tail(&self) -> u64 {
        let offset = std::mem::offset_of!(PerfEventMmapPage, data_tail);
        // SAFETY: see `data_head`. We own all writes to this field,
        // so a Relaxed load is enough for our own reads.
        unsafe {
            let ptr = self.base.add(offset) as *const AtomicU64;
            (*ptr).load(Ordering::Relaxed)
        }
    }

    /// Advance `data_tail`.
    /// Release-ordered: the kernel acquire-loads it to know how much
    /// of the ring it may reclaim.
    pub fn set_data_tail(&self, val: u64) {
        let offset = std::mem::offset_of!(PerfEventMmapPage, data_tail);
        // SAFETY: the field is 8-byte aligned at this offset; we own writes.
        unsafe {
            let ptr = self.base.add(offset) as *const AtomicU64;
            (*ptr).store(val, Ordering::Release);
        }
    }

    /// Consume every record the kernel has written since the last drain.
    /// Calls `on_record(&header, &body)` for each one, where `body` is
    /// the full record bytes (header + payload).
    ///
    /// Returns the number of records consumed.
    ///
    /// Handles ring-buffer wrap: when a record spans the end of the data
    /// buffer, this method copies it into a short-lived Vec before giving
    /// the slice to the callback.
    pub fn drain<F>(&self, mut on_record: F) -> usize
    where F: FnMut(&PerfEventHeader, &[u8]) {
        let head = self.data_head();
        let mut tail = self.data_tail();
        let data_base = self.data_ptr();
        let data_size = self.data_size();
        let mut count = 0usize;

        while tail < head {
            let offset = (tail as usize) % data_size;
            
            // Read header. The kernel guarantees it never splits a header.
            // SAFETY: offset..offset+8 is inside the ring; read_unaligned
            // tolerates arbitrary alignment inside the circular buffer.`
            let header: PerfEventHeader = unsafe {
                std::ptr::read_unaligned(data_base.add(offset) as *const PerfEventHeader)
            };
            let record_size = header.size as usize;

            if offset + record_size <= data_size {
                // Contiguous — borrow a slice directly out of the ring.
                // SAFETY: in-bounds; the mapping is live for 'self.
                let bytes = unsafe {
                    std::slice::from_raw_parts(data_base.add(offset), record_size)
                };
                on_record(&header, bytes);
            } else {
                // Record wraps around the end — assemble into a scratch Vec.
                let first = data_size - offset;
                let second: usize = record_size - first;
                let mut buf = Vec::<u8>::with_capacity(record_size);
                // SAFETY: both source ranges are inside the ring; `buf` is
                // owned by us and has reserved `record_size` bytes.
                unsafe {
                    buf.set_len(record_size);
                    std::ptr::copy_nonoverlapping(data_base.add(offset), buf.as_mut_ptr(), first);
                    std::ptr::copy_nonoverlapping(data_base, buf.as_mut_ptr().add(first), second);
                }
                on_record(&header, &buf);
            }
            tail = tail.wrapping_add(record_size as u64);
            count += 1;
            
        }
        
        self.set_data_tail(tail);
        count
    }

    pub fn wait(&self, timeout: c_int) -> io::Result<bool> {
        let mut pfd = PollFd {
            fd: self.fd.0,
            events: POLLIN,
            revents: 0,
        };
        // SAFETY: &mut pfd is a live, writable PollFd; nfds matches the slice length.
        let rc = unsafe { poll(&mut pfd, 1, timeout) };
        match rc {
            -1 => Err(io::Error::last_os_error()),
            0 => Ok(false), // timeout
            _ => {
                if pfd.revents & (POLLERR | POLLNVAL) != 0 {
                    return Err(io::Error::other("poll reported err on perf fd"));
                }
            // POLLHUP alone means the tracked task is gone — treat as readable
            // because there may still be records in the ring to drain.
                Ok(pfd.revents & (POLLIN | POLLHUP) != 0)
            }
        }
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.fd.0
    }
}

impl Drop for PerfRing {
    fn drop(&mut self) {
        // SAFETY: we own this mapping, pointer and length match the mmap call.
        unsafe { munmap(self.base as *mut c_void, self.total_len) };
    }
}

/// 8-byte prefix on every record written to the ring.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PerfEventHeader {
    pub type_: u32,
    pub misc: u16,
    pub size: u16,
}

/// Payload shape of `PERF_RECORD_FORK` and `PERF_RECORD_EXIT`
/// (identical layout). Use the `header.type_` discriminator to
/// know which kind of record you got.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskEventRecord {
    pub header: PerfEventHeader,
    pub pid: u32,
    pub ppid: u32,
    pub tid: u32,
    pub ptid: u32,
    pub time: u64,
}

impl TaskEventRecord {
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < std::mem::size_of::<Self>() { return None }
        Some( unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) })
    }
}

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: c_short,
    pub revents: c_short,

}

pub struct Poller {
    fds: Vec<PollFd>,
}

impl Poller {
    pub fn new() -> Self {
        Self { fds: Vec::new() }
    }

    /// Register an fd for POLLIN. Returns a token the caller uses to
    /// identify its source when iterating ready events
    pub fn register(&mut self, fd: RawFd) -> usize {
        self.fds.push(PollFd {
            fd,
            events: POLLIN,
            revents: 0,
        });
        self.fds.len() - 1
    }

    pub fn wait(&mut self, timeout_ms: c_int) -> io::Result<Readiness<'_>> {
        for pfd in &mut self.fds {
            pfd.revents = 0;
        };
        let rc = unsafe { poll(self.fds.as_mut_ptr(), self.fds.len() as c_ulong, timeout_ms) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Readiness { fds: &self.fds })
    }
}

pub struct Readiness<'a> {
    fds: &'a [PollFd]
}

impl<'a> Readiness<'a> {
    pub fn iter(&self) -> impl Iterator<Item = (usize, c_short)> + 'a {
        self.fds.iter().enumerate()
        .filter(|(_, p)| p.revents != 0)
        .map(|(i, p)| (i, p.revents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attr_size_is_sane() {
        // Current kernel struct size is 136 bytes on x86_64 Linux (as of 6.x).
        // We just assert it is in a sensible range and nonzero; the kernel
        // itself will reject an incorrect size at open-time.
        let s = std::mem::size_of::<PerfEventAttr>();
        assert!(s >= 128 && s <= 256, "unexpected PerfEventAttr size: {s}");
    }

    #[test]
    fn opens_and_closes_a_task_clock_counter() {
        let mut attr = PerfEventAttr::default();
        attr.type_  = PERF_TYPE_SOFTWARE;
        attr.config = PERF_COUNT_SW_TASK_CLOCK;

        let counter = PerfCounter::open(&mut attr, Pid(0), -1)
            .expect("TASK_CLOCK should open with SYS_ADMIN + paranoid=2");

        assert!(counter.as_raw_fd() >= 0);
        // Drop runs here — close(fd) is called by RAII.
    }

    #[test]
    fn task_tracker_opens() {
        let counter = PerfCounter::open_task_tracker(Pid(0))
            .expect("task-tracker counter should open on this process");
        assert!(counter.as_raw_fd() >= 0);
        // No records yet — we haven't mmap'd the ring, that's step 4.
    }

    #[test]
    fn task_tracker_mmaps_a_ring() {
        let counter = PerfCounter::open_task_tracker(Pid(0))
            .expect("task-tracker opens");
        let ring = counter.attach_ring(4) // 16 data pages
            .expect("mmap of perf fd should succeed");

        let hdr = ring.header();
        // Fresh ring: nothing consumed yet.
        assert_eq!(hdr.data_tail, 0, "data_tail starts at 0");
        // data_size is the data region we mapped (newer kernels fill this in).
        // Older kernels leave it 0 — tolerate both.
        if hdr.data_size != 0 {
            assert_eq!(hdr.data_size as usize, ring.data_size());
        }
        // Drop frees the mapping via munmap.
    }

    #[test]
    fn drain_empty_ring_calls_nothing() {
        let counter = PerfCounter::open_task_tracker(Pid(0)).unwrap();
        let ring = counter.attach_ring(4).unwrap();

        let mut seen = 0;
        let drained = ring.drain(|_hdr, _bytes| seen += 1);

        assert_eq!(seen, 0, "fresh ring should emit no records");
        assert_eq!(drained, 0);
    }

    #[test]
    fn poll_wakes_and_drain_sees_fork() {
        let counter = PerfCounter::open_task_tracker(Pid(0))
            .expect("task-tracker opens");
        let ring = counter.attach_ring(4).expect("mmap ring");

        // Before any fork: a short poll should time out.
        let ready = ring.wait(50).expect("poll");
        assert!(!ready, "no events before we fork");

        // Fork + exec via std. On modern Rust/Linux this uses posix_spawn,
        // which still invokes clone(2) under the hood — kernel emits FORK.
        let mut child = std::process::Command::new("/bin/true")
            .spawn()
            .expect("spawn /bin/true");
        let _ = child.wait().expect("wait /bin/true");

        // Now poll should wake quickly.
        let ready = ring.wait(1000).expect("poll");
        assert!(ready, "poll should have woken on FORK/EXIT activity");

        // Drain and look for the FORK record.
        let mut fork_pid: Option<u32> = None;
        let n = ring.drain(|hdr, bytes| {
            if hdr.type_ == PERF_RECORD_FORK {
                if let Some(rec) = TaskEventRecord::try_from_bytes(bytes) {
                    fork_pid = Some(rec.pid);
                }
            }
        });
        assert!(n > 0, "drain should have consumed at least one record");
        let pid = fork_pid.expect("a FORK record among the drained");
        assert!(pid > 1, "child pid should be sensible");
        eprintln!("saw FORK for child pid {pid}");
    }

    #[test]
    fn per_cpu_inherit_mmap_works() {
        // Open the same side-band tracker as open_task_tracker, but pinned
        // to CPU 0 and with ATTR_INHERIT. This is the combination we'd use
        // in taskmaster's per-CPU + inherit architecture.
        let mut attr = PerfEventAttr::default();
        attr.type_       = PERF_TYPE_SOFTWARE;
        attr.config      = PERF_COUNT_SW_DUMMY;
        attr.flags       = ATTR_TASK
                        | ATTR_COMM
                        | ATTR_INHERIT
                        | ATTR_EXCLUDE_KERNEL
                        | ATTR_WATERMARK;
        attr.wakeup      = 1;
        attr.sample      = 0;
        attr.sample_type = 0;
        attr.read_format = 0;

        let counter = PerfCounter::open(&mut attr, Pid(0), 0 /* CPU 0 */)
            .expect("per-CPU inherit counter should open");
        let ring = counter.attach_ring(4)
            .expect("mmap of per-CPU inherit counter");

        // If we got here, the exact architecture we'd use in taskmaster works.
        assert!(ring.data_head() == 0, "fresh ring has no records yet");
    }
}



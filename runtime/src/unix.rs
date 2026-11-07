use std::ffi::{c_int, c_short, c_void, c_ulong, c_uint};
use std::io;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicI32, Ordering};
#[cfg(test)]
use std::sync::Mutex;

pub const O_NONBLOCK: c_int = 0o4000;
pub const O_CLOEXEC: c_int = 0o2000000;
pub const READ_BUFFER_SIZE: usize = 100;
pub const POLLNVAL:c_short = 0x020;
pub const POLLHUP: c_short = 0x010;
pub const POLLIN: c_short = 0x001;
pub const POLLERR: c_short = 0x008;
pub const SIGCHLD: c_int = 17;
pub const SA_NOCLDSTOP: c_int = 1;
pub const WNOHANG: c_int = 1;
pub const ECHILD: c_int = 10;
pub const SIG_BLOCK: c_int = 0;
pub const SIG_SETMASK: c_int  = 2;

static SIGCHLD_WRITE_FD: AtomicI32  = AtomicI32::new(-1);

#[cfg(test)]
pub(crate) static SIGNAL_HANDLER_TEST_LOCK: Mutex<()> = Mutex::new(());

#[repr(C)]
struct PollFd {
	fd: c_int,
	events: c_short,
	revents: c_short
}


#[repr(C)]
union SignalHandler {
	handler: extern "C" fn(c_int)
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct SigSet {
	value: [c_ulong;16],
}

#[repr(C)]
struct SigAction {
	sa_handler: SignalHandler,
	mask: SigSet,
	flags: c_int,
	restorer: Option<extern "C" fn()>
}

pub struct SigChldMaskGuard{
	previous_mask: SigSet
}

#[link(name = "c")]
unsafe extern "C" {
	fn pipe2(file_descriptors: *mut c_int, flags: c_int) -> c_int;
	fn close(file_descriptor: c_int) -> c_int;
	fn read(file_descriptor: c_int, buffer: *mut c_void, count: usize) -> isize;
	fn write(file_descriptor: c_int,buffer : *const c_void, count: usize ) -> isize;
	fn poll(fds: *mut PollFd, descriptor_count: usize, timeout: c_int) -> c_int;
	fn sigemptyset(set: *mut SigSet) -> c_int;
	fn sigaction(sig: c_int, sighandler:*const SigAction, previous_action: *mut SigAction) -> c_int;
	fn __errno_location() -> *mut c_int;
	fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
	fn sigaddset(set: *mut SigSet, signum: c_int) -> c_int;
	fn sigprocmask(how: c_int, set: *const SigSet, old_set: *mut SigSet) -> c_int;
	#[cfg(test)]
	fn sigismember(set: *const SigSet, signum: c_int) -> c_int;
	#[cfg(test)]
	fn raise(signum: c_int) -> c_int;
	fn umask(mask: c_uint) -> c_uint;
}

pub fn set_umask(mask: u16) {
	unsafe {
		umask(mask.into());
	}
}

pub struct SelfPipe {
	read_fd: RawFd,
	write_fd: RawFd
}

impl SelfPipe {
	pub fn new() -> io::Result<Self> {
		let mut file_descriptors:[c_int; 2] = [-1, -1];

		let result = unsafe {
			pipe2(file_descriptors.as_mut_ptr(), O_NONBLOCK | O_CLOEXEC)
		};
		if result == -1 {
			return Err(io::Error::last_os_error());
		};
		Ok(Self { read_fd: file_descriptors[0], write_fd: file_descriptors[1] })
	}

	pub fn read_fd(&self) -> RawFd {
		self.read_fd
	}

	pub fn notify(&self)-> io::Result<()> {
		let notification_byte : [u8; 1] = [1];
		let result = unsafe {
			write(self.write_fd, notification_byte.as_ptr() as *const c_void, 1)
		};
		return match result {
			r if r == 0 => Err( io::Error::new(io::ErrorKind::WriteZero,
				"Self-pipe notification was not fully written")
			),
			r if r < 0 => Err(io::Error::last_os_error()),
			_ => Ok(())
		};
	}

	pub fn drain(&self) -> io::Result<usize> {
		let mut drained_bytes: [u8; READ_BUFFER_SIZE] = [0;READ_BUFFER_SIZE];
		let mut total_bytes_read = 0;
		loop {
			let result = unsafe {
				read(self.read_fd, drained_bytes.as_mut_ptr() as *mut c_void , drained_bytes.len())
			};
			match result {
				r if  r >= 0 => { 
					total_bytes_read += r as usize;
					if r == 0 {
						return Ok(total_bytes_read)
					}
					continue;
				},
				_ => ()
			};

			let error = io::Error::last_os_error();
			return match error.kind() {
				io::ErrorKind::WouldBlock => Ok(total_bytes_read),
				io::ErrorKind::Interrupted => continue,
				_ => Err(error)
			}
		}
	}

	pub fn wait_readable(&self, timeout_milliseconds: c_int) -> io::Result<bool> {
		let mut poll_fd = PollFd{
			fd: self.read_fd,
			events: POLLIN,
			revents: 0
		};
		loop {
			let result = unsafe {
				poll(&mut poll_fd,
					1,
					timeout_milliseconds				
				)
			};
			return match result {
				r if r > 0  && (poll_fd.revents & POLLIN != 0) => Ok(true),
				r if r > 0 && (poll_fd.revents & (POLLERR | POLLHUP | POLLNVAL) != 0) => Err(io::Error::other("self-pipe returned an unexpected poll event")),
				r if r >= 0 => Ok(false),
				r if r < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted => continue,
				_ => Err(io::Error::last_os_error())
			}
		}
	}
}

impl Drop for SelfPipe {
	fn drop(&mut self) {
		unsafe {
			close(self.read_fd);
			close(self.write_fd);

		}
	}
}

pub fn install_sigchld_handler(handler: extern "C" fn(c_int)) -> io::Result<()>{
	let mut mask = SigSet { value: [0;16] };
	let empty_set_result = unsafe {
		sigemptyset(&mut mask)
	};
	if empty_set_result == -1 {
		return Err(io::Error::last_os_error());
	}
	let action = SigAction {
		sa_handler: SignalHandler{
			handler
		},
		mask,
		flags: SA_NOCLDSTOP,
		restorer: None
	};
	let result = unsafe {
		sigaction(SIGCHLD, &action, std::ptr::null_mut())
	};
	if result == -1 {
		return Err(io::Error::last_os_error());
	}
	Ok(())
}


fn set_sigchld_write_fd(write_fd: RawFd){
	SIGCHLD_WRITE_FD.store(write_fd, Ordering::Relaxed);
}

pub fn cleanup_sigchld_pipe(){
	set_sigchld_write_fd(-1);
}

extern "C" fn sigchld_handler(_signal: c_int){
	let write_fd: c_int = SIGCHLD_WRITE_FD.load(Ordering::Relaxed);
	let errno_location = unsafe { __errno_location() };
    let saved_errno = unsafe { *errno_location };
	if write_fd == -1 { return };
	let write_content:[u8;1] = [1];
	unsafe {
		write(write_fd, write_content.as_ptr() as *const c_void, 1)
	};
	unsafe {
		*errno_location = saved_errno;
	}
	
}

pub fn configure_sigchld_notifications(pipe:&SelfPipe) -> io::Result<()> {
	set_sigchld_write_fd(pipe.write_fd);
	if let Err(e) = install_sigchld_handler(sigchld_handler) {
		cleanup_sigchld_pipe();
		return Err(e);
	}
	Ok(())
}


macro_rules! wif_exited {
	($status: expr) => {
		(($status & 0x7f) == 0 )
	};
}

macro_rules! wexit_status {
	($status: expr) => {
		(($status >> 8) & 0xff)
	};
}

macro_rules! wif_signaled {
    ($status:expr) => {
        (($status & 0x7f) != 0 && ($status & 0x7f) != 0x7f)
    };
}

macro_rules! wterm_sig {
	($status: expr) => {
		($status & 0x7f)
	};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTermination {
	Exited {code : u8},
	Signaled {signal: c_int}
}

impl ChildTermination {
	fn from_status(status: c_int) -> io::Result<Self> {
		let termination_state = match status {
			s if wif_exited!(s) => Self::Exited { code: wexit_status!(s) as u8 },
			s if  wif_signaled!(s) => Self::Signaled { signal: wterm_sig!(s)},
			_ => return Err(io::Error::new(io::ErrorKind::InvalidData, "No valid status reaped"))
		};
		Ok(termination_state)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReapedChild {
	pub pid: u32,
	pub termination: ChildTermination
}

pub fn reap_exited_children() -> io::Result<Vec<ReapedChild>> {
	let mut reaped_children: Vec<ReapedChild> = Vec::new();
	loop {
		let mut status= 0 ;
		let pid = unsafe {
			waitpid(-1, &mut status, WNOHANG)
		};
		match pid {
			p if p > 0 => {
				let termination = ChildTermination::from_status(status)?;
				let child_state = ReapedChild {
					pid: pid as u32,
					termination
				};
				reaped_children.push(child_state);
				continue
			}
			p if p == 0 => {
				return Ok(reaped_children);
			}
			_ => {
				let last_os_error = io::Error::last_os_error();
				match last_os_error {
					e if e.kind() == io::ErrorKind::Interrupted  => {
						continue;
					},
					e if e.raw_os_error() == Some(ECHILD) => {
						return Ok(reaped_children);
					},
					_ => return Err(last_os_error)
				}
			}
		}
	}
}



impl SigChldMaskGuard {
	pub fn block() -> io::Result<Self> {
		let mut previous_mask = SigSet{ value: [0;16] };
		match unsafe { sigemptyset(&mut previous_mask) } {
			-1 => return Err(io::Error::last_os_error()),
			_ => ()
		};
		let mut to_be_masked = previous_mask.clone();

		match unsafe { sigaddset(&mut to_be_masked, SIGCHLD) } {
			-1 => return Err(io::Error::last_os_error()),
			_ => ()
		};

		match unsafe { sigprocmask(SIG_BLOCK, &to_be_masked, &mut previous_mask) } {
			-1 => return Err(io::Error::last_os_error()),
			_ => ()
		};
		return Ok(Self {previous_mask});
	}
}

impl Drop for SigChldMaskGuard {
	fn drop(&mut self) {
		unsafe {
			sigprocmask(SIG_SETMASK, &self.previous_mask, std::ptr::null_mut())
		};
	}
}


#[cfg(test)]
mod tests {
	use super::{cleanup_sigchld_pipe, configure_sigchld_notifications, install_sigchld_handler, raise, reap_exited_children, sigaction, sigismember, sigprocmask, ChildTermination, SelfPipe, SigAction, SigChldMaskGuard, SigSet, SIGNAL_HANDLER_TEST_LOCK, SIG_BLOCK, SIGCHLD};
	use std::io;
	use std::mem::MaybeUninit;
	use std::process::Command;
	struct RestoreSigchldAction {
		action: SigAction,
	}

	struct ClearSigchldPipe;
  
	impl Drop for ClearSigchldPipe {
		fn drop(&mut self) {
			cleanup_sigchld_pipe();
		}
	}

	impl Drop for RestoreSigchldAction {
		fn drop(&mut self) {
			// SAFETY: action contains a disposition returned by sigaction for
			// SIGCHLD and remains valid for the duration of this restoration call.
			unsafe {
				sigaction(SIGCHLD, &self.action, std::ptr::null_mut());
			}
		}
	}

	fn current_sigchld_action() -> io::Result<SigAction> {
		let mut action = MaybeUninit::<SigAction>::uninit();

		// SAFETY: a null new-action asks sigaction to leave SIGCHLD unchanged;
		// action points to writable storage for the previous disposition.
		let result = unsafe {
			sigaction(SIGCHLD, std::ptr::null(), action.as_mut_ptr())
		};

		if result == -1 {
			return Err(io::Error::last_os_error());
		}

		// SAFETY: sigaction returned success and initialized action fully.
		Ok(unsafe { action.assume_init() })
	}

	fn sigchld_is_blocked() -> io::Result<bool> {
		let mut current_mask = MaybeUninit::<SigSet>::uninit();

		// SAFETY: a null set asks sigprocmask not to alter the current mask;
		// current_mask is writable storage for the mask returned by the kernel.
		let result = unsafe {
			sigprocmask(SIG_BLOCK, std::ptr::null(), current_mask.as_mut_ptr())
		};
		if result == -1 {
			return Err(io::Error::last_os_error());
		}

		// SAFETY: sigprocmask returned success and initialized current_mask.
		let current_mask = unsafe { current_mask.assume_init() };
		// SAFETY: current_mask is a valid signal set returned by sigprocmask.
		let result = unsafe { sigismember(&current_mask, SIGCHLD) };
		if result == -1 {
			return Err(io::Error::last_os_error());
		}

		Ok(result == 1)
	}

	extern "C" fn test_sigchld_handler(_signal: i32) {}

	#[test]
	fn creates_two_distinct_valid_file_descriptors() {
		let pipe = SelfPipe::new().expect("pipe2 must create a self-pipe");

		assert!(pipe.read_fd >= 0);
		assert!(pipe.write_fd >= 0);
		assert_ne!(pipe.read_fd, pipe.write_fd);
	}

	#[test]
	fn notifies_and_drains_the_self_pipe() {
		let pipe = SelfPipe::new().expect("pipe2 must create a self-pipe");

		pipe.notify()
			.expect("self-pipe notification must succeed");

		assert_eq!(
			pipe.drain().expect("self-pipe drain must succeed"),
			1
		);
		assert_eq!(
			pipe.drain().expect("empty self-pipe drain must succeed"),
			0
		);
	}

	#[test]
	fn waits_for_a_self_pipe_notification() {
		let pipe = SelfPipe::new()
			.expect("pipe2 must create a self-pipe");

		assert!(
			!pipe.wait_readable(0)
				.expect("empty self-pipe must be checked")
		);

		pipe.notify()
			.expect("self-pipe notification must succeed");

		assert!(
			pipe.wait_readable(100)
				.expect("notified self-pipe must become readable")
		);

		assert_eq!(
			pipe.drain().expect("self-pipe drain must succeed"),
			1
		);
	}

	#[test]
	fn installs_a_sigchld_handler() {
		let _lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let previous_action = current_sigchld_action()
			.expect("current SIGCHLD action must be readable");
		let _restore_action = RestoreSigchldAction {
			action: previous_action,
		};

		install_sigchld_handler(test_sigchld_handler)
			.expect("SIGCHLD handler installation must succeed");
	}

	#[test]
	fn sigchld_mask_guard_blocks_and_restores_sigchld() {
		let _lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let was_blocked = sigchld_is_blocked()
			.expect("current signal mask must be readable");

		{
			let _guard = SigChldMaskGuard::block()
				.expect("SIGCHLD mask guard must be created");
			assert!(
				sigchld_is_blocked().expect("SIGCHLD must be readable from signal mask")
			);
		}

		assert_eq!(
			sigchld_is_blocked().expect("restored signal mask must be readable"),
			was_blocked
		);
	}

	#[test]
	fn decodes_normal_and_signal_termination_statuses() {
		assert_eq!(
			ChildTermination::from_status(42 << 8)
				.expect("normal exit status must decode"),
			ChildTermination::Exited { code: 42 },
		);
		assert_eq!(
			ChildTermination::from_status(15)
				.expect("signal termination status must decode"),
			ChildTermination::Signaled { signal: 15 },
		);
	}

	#[test]
	fn child_exit_notifies_the_self_pipe() {
		let _lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let pipe = SelfPipe::new().expect("pipe2 must create a self-pipe");
		let previous_action = current_sigchld_action()
			.expect("current SIGCHLD action must be readable");
		let _restore_action = RestoreSigchldAction {
			action: previous_action,
		};
		let _clear_pipe = ClearSigchldPipe;

		configure_sigchld_notifications(&pipe)
			.expect("SIGCHLD notifications must be configured");

		let mut child = Command::new("/usr/bin/true")
			.spawn()
			.expect("test child must start");

		let readiness = pipe.wait_readable(1_000);
		let is_readable = matches!(readiness.as_ref(), Ok(true));
		let drained = if is_readable {
			pipe.drain()
		} else {
			Ok(0)
		};
		let status = child.wait().expect("test child must be reaped");

		assert!(readiness.expect("waiting for SIGCHLD notification must succeed"));
		assert!(drained.expect("self-pipe drain must succeed") >= 1);
		assert!(status.success());
	}

	#[test]
	fn reaps_a_normally_exited_child() {
		let _lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let pipe = SelfPipe::new().expect("pipe2 must create a self-pipe");
		let previous_action = current_sigchld_action()
			.expect("current SIGCHLD action must be readable");
		let _restore_action = RestoreSigchldAction {
			action: previous_action,
		};
		let _clear_pipe = ClearSigchldPipe;

		configure_sigchld_notifications(&pipe)
			.expect("SIGCHLD notifications must be configured");

		let child = Command::new("/usr/bin/true")
			.spawn()
			.expect("test child must start");
		let child_pid = child.id();

		assert!(
			pipe.wait_readable(1_000)
				.expect("child exit must notify the self-pipe")
		);
		pipe.drain().expect("self-pipe drain must succeed");

		let reaped = reap_exited_children()
			.expect("exited child must be reaped");

		assert!(reaped.iter().any(|child| {
			child.pid == child_pid
				&& child.termination == ChildTermination::Exited { code: 0 }
		}));
		assert!(
			reap_exited_children()
				.expect("second reap must succeed")
				.is_empty()
		); 
	}

	#[test]
	fn defers_thread_directed_sigchld_until_mask_guard_is_dropped() {
		let _lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		assert!(
			!sigchld_is_blocked().expect("current signal mask must be readable"),
			"test requires SIGCHLD to be initially unblocked"
		);
		let pipe = SelfPipe::new().expect("pipe2 must create a self-pipe");
		let previous_action = current_sigchld_action()
			.expect("current SIGCHLD action must be readable");
		let _restore_action = RestoreSigchldAction {
			action: previous_action,
		};
		let _clear_pipe = ClearSigchldPipe;

		configure_sigchld_notifications(&pipe)
			.expect("SIGCHLD notifications must be configured");

		let guard = SigChldMaskGuard::block()
			.expect("SIGCHLD mask guard must be created");
		// SAFETY: raise sends SIGCHLD to this test thread, where the guard has
		// blocked it. The configured handler is safe for SIGCHLD delivery.
		assert_eq!(unsafe { raise(SIGCHLD) }, 0);

		assert!(
			!pipe.wait_readable(100)
				.expect("blocked SIGCHLD must not notify the self-pipe")
		);

		drop(guard);

		assert!(
			pipe.wait_readable(1_000)
				.expect("unblocked SIGCHLD must notify the self-pipe")
		);
		pipe.drain().expect("self-pipe drain must succeed");
	}
}


use std::ffi::{c_int, c_short, c_void, c_ulong};
use std::io;
use std::os::fd::RawFd;

pub const O_NONBLOCK: c_int = 0o4000;
pub const O_CLOEXEC: c_int = 0o2000000;
pub const READ_BUFFER_SIZE: usize = 100;
pub const POLLNVAL:c_short = 0x020;
pub const POLLHUP: c_short = 0x010;
pub const POLLIN: c_short = 0x001;
pub const POLLERR: c_short = 0x008;
pub const SIGCHLD: c_int = 17;
pub const SA_NOCLDSTOP: c_int = 1;

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

#[link(name = "c")]
unsafe extern "C" {
	fn pipe2(file_descriptors: *mut c_int, flags: c_int) -> c_int;
	fn close(file_descriptor: c_int) -> c_int;
	fn read(file_descriptor: c_int, buffer: *mut c_void, count: usize) -> isize;
	fn write(file_descriptor: c_int,buffer : *const c_void, count: usize ) -> isize;
	fn poll(fds: *mut PollFd, descriptor_count: usize, timeout: c_int) -> c_int;
	fn sigemptyset(set: *mut SigSet) -> c_int;
	fn sigaction(sig: c_int, sighandler:*const SigAction, previous_action: *mut SigAction) -> c_int;
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


#[cfg(test)]
mod tests {
	use super::{install_sigchld_handler, sigaction, SelfPipe, SigAction, SIGCHLD};
	use std::io;
	use std::mem::MaybeUninit;
	use std::sync::Mutex;

	static SIGNAL_HANDLER_TEST_LOCK: Mutex<()> = Mutex::new(());

	struct RestoreSigchldAction {
		action: SigAction,
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
}


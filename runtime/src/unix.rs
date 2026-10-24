use std::ffi::{c_int, c_void};
use std::io;
use std::os::fd::RawFd;

pub const O_NONBLOCK: c_int = 0o4000;
pub const O_CLOEXEC: c_int = 0o2000000;
pub const READ_BUFFER_SIZE: usize = 100;


#[link(name = "c")]
unsafe extern "C" {
	fn pipe2(file_descriptors: *mut c_int, flags: c_int) -> c_int;
	fn close(file_descriptor: c_int) -> c_int;
	fn read(file_descriptor: c_int, buffer: *mut c_void, count: usize) -> isize;
	fn write(file_descriptor: c_int,buffer : *const c_void, count: usize ) -> isize;
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
		if result != 1 {
			return  Err(io::Error::last_os_error());
		}
		Ok(())
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
}

impl Drop for SelfPipe {
	fn drop(&mut self) {
		unsafe {
			close(self.read_fd);
			close(self.write_fd);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::SelfPipe;

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
}


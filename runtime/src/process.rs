use std::process::{Command, Stdio};
use config_parser::config::ProgramConfig;
use crate::unix::{ChildTermination, set_umask};
use std::error::Error;
use std::os::unix::process::CommandExt;


pub enum RuntimeProcessState {
	NotStarted,
	Running {pid: u32},
	Exited {termination : ChildTermination}
}

pub struct RuntimeProcess<'a> {
	config: &'a ProgramConfig,
	status: RuntimeProcessState
}


#[derive(Debug)]
pub enum RuntimeProcessError {
	DEFAULT(String),
	AlreadyRunning,
	NotRunningChildTerminated,
	PIDMismatch(u32)
}

impl Error for RuntimeProcessError {}


impl std::fmt::Display for RuntimeProcessError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DEFAULT(s) => write!(f,"Runtime Process Error: {}", s),
			Self::AlreadyRunning => write!(f, "Not allowed to restart same process"),
			Self::NotRunningChildTerminated => write!(f, "Child terminated when not running"),
			Self::PIDMismatch(pid) => write!(f, "Cannot terminate mismatched pid: {}", pid)
		}
	}
}


impl From<std::io::Error> for RuntimeProcessError {
	fn from(value: std::io::Error) -> Self {
		return  Self::DEFAULT(value.to_string());
	}	
}


type RuntimeProcessResult<T> = Result<T,RuntimeProcessError>;



impl<'a> RuntimeProcess<'a>{

	pub fn new(config: &'a ProgramConfig) -> Self {
		let status = RuntimeProcessState::NotStarted;
		return Self {
			config,
			status
		};
	}

	pub fn start(&mut self) -> RuntimeProcessResult<u32> {
		match self.status {
			RuntimeProcessState::Running { .. } 
				=> return Err(RuntimeProcessError::AlreadyRunning),
			_ => ()
		};
		let mut command = Command::new(&self.config.command.path);
		command.args(&self.config.command.args);
		command.current_dir(&self.config.chdir.path);
		if let Some(var) = self.config.env.as_ref() {
			command.envs(&var.values);
		}
		match &self.config.stdout {
			Some(target) => command.stdout(target.open_stream()?),
			None => command.stdout(Stdio::null()) 
		};
		match &self.config.stderr {
			Some(target) => command.stderr(target.open_stream()?),
			None => command.stderr(Stdio::null())
		};	
		if let Some(mask) = self.config.umask.as_ref().map(|mask| mask.value){
			unsafe {
				command.pre_exec(move || {
					set_umask(mask);
					Ok(())
				});
			}
		}
		let child = command.spawn()?;
		let pid = child.id();
		self.status = RuntimeProcessState::Running { pid };
		Ok(pid)
	}

	pub fn state(&self) -> &RuntimeProcessState {
		&self.status
	}

	pub fn pid(&self) -> Option<u32> {
		match self.status {
			RuntimeProcessState::Exited { .. } | RuntimeProcessState::NotStarted => None,
			RuntimeProcessState::Running { pid } => Some(pid)
		}
	}

	pub fn is_running(&self) -> bool {
		match self.status {
			RuntimeProcessState::Exited { .. } | RuntimeProcessState::NotStarted => false,
			RuntimeProcessState::Running { .. } => true
		}
	}

	pub fn matches_pid(&self, pid: u32) -> bool {
		if let Some(curr_pid) = self.pid() && curr_pid == pid {
			return true;
		}
		false
	}

	pub fn record_exit(&mut self,pid: u32, termination : ChildTermination) -> RuntimeProcessResult<()> {
		match self.status {
			RuntimeProcessState::Exited { .. } | RuntimeProcessState::NotStarted => Err(RuntimeProcessError::NotRunningChildTerminated),
			RuntimeProcessState::Running { .. } if self.matches_pid(pid)  => {
				self.status = RuntimeProcessState::Exited { termination };
				Ok(())
			}
			RuntimeProcessState::Running { .. } => Err(RuntimeProcessError::PIDMismatch(pid))
		}
	}
	
}

#[cfg(test)]
mod tests {
	use super::{RuntimeProcess, RuntimeProcessError, RuntimeProcessState};
	use crate::unix::{ChildTermination, SIGNAL_HANDLER_TEST_LOCK};
	use config_parser::config::ProgramConfig;
	use config_parser::validators::{
		Command, EnvironmentVariables, ExitCodes, ExitSignals, ProcessCount, RestartPolicy, Stderr, Stdout, Umask, WorkingDir,
	};
	use std::collections::HashMap;
	use std::fs;
	use std::os::unix::fs::PermissionsExt;
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	unsafe extern "C" {
		fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
	}

	fn temporary_path(name: &str) -> PathBuf {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock must be after the Unix epoch")
			.as_nanos();

		std::env::temp_dir().join(format!(
			"taskmaster-runtime-process-{name}-{}-{timestamp}",
			std::process::id()
		))
	}

	#[test]
	fn start_applies_the_program_configuration() {
		let _signal_handler_lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let directory = temporary_path("start");
		fs::create_dir(&directory).expect("temporary directory must be created");
		let executable = directory.join("program.sh");
		let stdout_path = directory.join("output.txt");
		let stderr_path = directory.join("error.txt");
		let child_file_path = directory.join("created-by-child");

		fs::write(
			&executable,
			"#!/bin/sh\nprintf '%s\\n' \"$TASKMASTER_PROCESS_TEST\"\nprintf '%s\\n' \"$1\"\npwd\nprintf '%s\\n' 'configured error' >&2\n: > created-by-child\n",
		)
		.expect("test executable must be written");
		fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
			.expect("test executable must be executable");

		let config = ProgramConfig {
			command: Command::try_from(format!("{} configured-argument", executable.display()))
				.expect("test command must be valid"),
			chdir: WorkingDir::try_from(directory.to_string_lossy().into_owned())
				.expect("test directory must be valid"),
			umask: Some(Umask::try_from("027".to_string())
				.expect("test umask must be valid")),
			process_count: ProcessCount::try_from(1).expect("one process must be valid"),
			auto_start: true,
			restart_protocol: RestartPolicy::Never,
			exit_codes: ExitCodes::try_from(vec![0]).expect("exit code must be valid"),
			runtime: 0,
			restart_count: 0,
			exit_signal: ExitSignals::Term,
			kill_time: 0,
			stdout: Some(Stdout::try_from(stdout_path.to_string_lossy().into_owned())
				.expect("test output must be valid")),
			stderr: Some(Stderr::try_from(stderr_path.to_string_lossy().into_owned())
				.expect("test error output must be valid")),
			env: Some(EnvironmentVariables::try_from(HashMap::from([(
				"TASKMASTER_PROCESS_TEST".to_string(),
				"configured environment".to_string(),
			)]))
			.expect("test environment must be valid")),
		};

		let mut process = RuntimeProcess::new(&config);
		let pid = process.start().expect("configured process must start");

		assert!(matches!(process.status, RuntimeProcessState::Running { pid: running_pid } if running_pid == pid));
		assert_eq!(process.pid(), Some(pid));
		assert!(process.is_running());
		assert!(process.matches_pid(pid));
		assert!(!process.matches_pid(pid + 1));
		assert!(matches!(
			process.start(),
			Err(RuntimeProcessError::AlreadyRunning)
		));
		assert!(matches!(
			process.record_exit(pid + 1, ChildTermination::Exited { code: 0 }),
			Err(RuntimeProcessError::PIDMismatch(mismatched_pid)) if mismatched_pid == pid + 1
		));
		assert!(matches!(process.state(), RuntimeProcessState::Running { pid: running_pid } if *running_pid == pid));

		let mut wait_status = 0;
		let waited_pid = unsafe { waitpid(pid as i32, &mut wait_status, 0) };
		assert_eq!(waited_pid, pid as i32, "the started child must be reaped");
		assert_eq!(wait_status, 0, "the test child must exit successfully");
		assert_eq!(
			fs::read_to_string(&stdout_path).expect("configured stdout must be readable"),
			format!(
				"configured environment\nconfigured-argument\n{}\n",
				directory.display()
			),
		);
		assert_eq!(
			fs::read_to_string(&stderr_path).expect("configured stderr must be readable"),
			"configured error\n",
		);
		assert_eq!(
			fs::metadata(&child_file_path)
				.expect("child-created file must exist")
				.permissions()
				.mode() & 0o777,
			0o640,
			"the child-created file must use the configured umask"
		);
		process
			.record_exit(pid, ChildTermination::Exited { code: 0 })
			.expect("matching running PID must transition to exited");
		assert!(matches!(
			process.state(),
			RuntimeProcessState::Exited {
				termination: ChildTermination::Exited { code: 0 }
			}
		));
		assert_eq!(process.pid(), None);
		assert!(!process.is_running());
		assert!(matches!(
			process.record_exit(pid, ChildTermination::Exited { code: 0 }),
			Err(RuntimeProcessError::NotRunningChildTerminated)
		));

		let restarted_pid = process.start().expect("an exited process must restart");
		let restarted_waited_pid = unsafe { waitpid(restarted_pid as i32, &mut wait_status, 0) };
		assert_eq!(restarted_waited_pid, restarted_pid as i32, "restarted child must be reaped");
		process
			.record_exit(restarted_pid, ChildTermination::Exited { code: 0 })
			.expect("restarted child must record its matching exit");

		fs::remove_dir_all(&directory).expect("temporary test directory must be removed");
	}

	#[test]
	fn start_discards_omitted_stdout_and_stderr() {
		let _signal_handler_lock = SIGNAL_HANDLER_TEST_LOCK
			.lock()
			.expect("signal-handler test lock must not be poisoned");
		let directory = temporary_path("discard-output");
		fs::create_dir(&directory).expect("temporary directory must be created");
		let executable = directory.join("program.sh");

		fs::write(
			&executable,
			"#!/bin/sh\nprintf '%s\\n' 'discard stdout'\nprintf '%s\\n' 'discard stderr' >&2\n",
		)
		.expect("test executable must be written");
		fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
			.expect("test executable must be executable");

		let config = ProgramConfig {
			command: Command::try_from(executable.to_string_lossy().into_owned())
				.expect("test command must be valid"),
			chdir: WorkingDir::try_from(directory.to_string_lossy().into_owned())
				.expect("test directory must be valid"),
			umask: None,
			process_count: ProcessCount::try_from(1).expect("one process must be valid"),
			auto_start: true,
			restart_protocol: RestartPolicy::Never,
			exit_codes: ExitCodes::try_from(vec![0]).expect("exit code must be valid"),
			runtime: 0,
			restart_count: 0,
			exit_signal: ExitSignals::Term,
			kill_time: 0,
			stdout: None,
			stderr: None,
			env: Some(EnvironmentVariables::try_from(HashMap::new())
				.expect("an empty environment must be valid")),
		};

		let mut process = RuntimeProcess::new(&config);
		let pid = process.start().expect("process with discarded output must start");
		let mut wait_status = 0;
		let waited_pid = unsafe { waitpid(pid as i32, &mut wait_status, 0) };

		assert_eq!(waited_pid, pid as i32, "discarding output must not affect execution");
		assert_eq!(wait_status, 0, "discard-output child must exit successfully");
		assert!(
			!directory.join("stdout.log").exists() && !directory.join("stderr.log").exists(),
			"omitted output targets must not create default log files"
		);

		fs::remove_dir_all(&directory).expect("temporary test directory must be removed");
	}
}


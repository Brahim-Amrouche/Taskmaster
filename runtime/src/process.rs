use std::process::{Command};
use config_parser::config::ProgramConfig;
use crate::unix::ChildTermination;
use std::error::Error;


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
	NotAllowedProcessRestart,
	NotRunningChildTerminated,
}

impl Error for RuntimeProcessError {}


impl std::fmt::Display for RuntimeProcessError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DEFAULT(s) => write!(f,"Runtime Process Error: {}", s),
			Self::NotAllowedProcessRestart => write!(f, "Not allowed to restart same process"),
			Self::NotRunningChildTerminated => write!(f, "Child terminated when not running")
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
				=> return Err(RuntimeProcessError::NotAllowedProcessRestart),
			_ => ()
		};
		let mut command = Command::new(&self.config.command.path);
		command.args(&self.config.command.args);
		command.current_dir(&self.config.chdir.path);
		command.envs(&self.config.env);
		command.stdin(self.config.stdin.open_stream()?);
		command.stdout(self.config.stdout.open_stream()?);
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

	pub fn record_exit(&mut self, termination : ChildTermination) -> RuntimeProcessResult<()> {
		match self.status {
			RuntimeProcessState::Exited { .. } | RuntimeProcessState::NotStarted => Err(RuntimeProcessError::NotRunningChildTerminated),
			RuntimeProcessState::Running { .. } => {
				self.status = RuntimeProcessState::Exited { termination };
				Ok(())
			}
		}
	}
	
}

#[cfg(test)]
mod tests {
	use super::{RuntimeProcess, RuntimeProcessState};
	use crate::unix::SIGNAL_HANDLER_TEST_LOCK;
	use config_parser::config::ProgramConfig;
	use config_parser::validators::{
		Command, ExitCodes, ExitSignals, ProcessCount, RestartPolicy, Stdin, Stdout, WorkingDir,
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
		let stdin_path = directory.join("input.txt");
		let stdout_path = directory.join("output.txt");

		fs::write(
			&executable,
			"#!/bin/sh\nprintf '%s\\n' \"$TASKMASTER_PROCESS_TEST\"\nprintf '%s\\n' \"$1\"\npwd\ncat\n",
		)
		.expect("test executable must be written");
		fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
			.expect("test executable must be executable");
		fs::write(&stdin_path, "configured input\n")
			.expect("test input must be written");

		let config = ProgramConfig {
			command: Command::try_from(format!("{} configured-argument", executable.display()))
				.expect("test command must be valid"),
			chdir: WorkingDir::try_from(directory.to_string_lossy().into_owned())
				.expect("test directory must be valid"),
			chmod: None,
			process_count: ProcessCount::try_from(1).expect("one process must be valid"),
			auto_start: true,
			restart_protocol: RestartPolicy::Never,
			exit_codes: ExitCodes::try_from(vec![0]).expect("exit code must be valid"),
			runtime: 0,
			restart_count: 0,
			exit_signal: ExitSignals::Term,
			kill_time: 0,
			stdin: Stdin::try_from(stdin_path.to_string_lossy().into_owned())
				.expect("test input must be valid"),
			stdout: Stdout::try_from(stdout_path.to_string_lossy().into_owned())
				.expect("test output must be valid"),
			env: HashMap::from([(
				"TASKMASTER_PROCESS_TEST".to_string(),
				"configured environment".to_string(),
			)]),
		};

		let mut process = RuntimeProcess::new(&config);
		let pid = process.start().expect("configured process must start");

		assert!(matches!(process.status, RuntimeProcessState::Running { pid: running_pid } if running_pid == pid));

		let mut wait_status = 0;
		let waited_pid = unsafe { waitpid(pid as i32, &mut wait_status, 0) };
		assert_eq!(waited_pid, pid as i32, "the started child must be reaped");
		assert_eq!(wait_status, 0, "the test child must exit successfully");
		assert_eq!(
			fs::read_to_string(&stdout_path).expect("configured stdout must be readable"),
			format!(
				"configured environment\nconfigured-argument\n{}\nconfigured input\n",
				directory.display()
			),
		);

		fs::remove_dir_all(&directory).expect("temporary test directory must be removed");
	}
}


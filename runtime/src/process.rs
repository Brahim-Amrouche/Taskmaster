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
	DEFAULT(String)
}

impl Error for RuntimeProcessError {}


impl std::fmt::Display for RuntimeProcessError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DEFAULT(s) => write!(f,"Runtime Process Error: {}", s)
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

}


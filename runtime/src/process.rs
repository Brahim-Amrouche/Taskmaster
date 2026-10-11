use std::process::{ExitStatus, Command};
use config_parser::config::ProgramConfig;
use std::error::Error;

pub struct RuntimeProcess<'a> {
	config: &'a ProgramConfig,
	pid: u32,
	exit_status: Option<ExitStatus>
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
		return Self {config, pid: 0, exit_status: None };
	}

	pub fn run(&mut self) -> RuntimeProcessResult<()> {
		let out = Command::new(&self.config.command.path)
		.output()?;
		println!("{}",String::from_utf8_lossy(&out.stdout.to_ascii_lowercase()));
		Ok(())
	}
}


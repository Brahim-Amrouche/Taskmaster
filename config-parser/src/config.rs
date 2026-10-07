use std::collections::HashMap;
use serde::Deserialize;
use crate::validators::{
	Command, ProcessCount, Stdin, Stdout, Umask, WorkingDir, RestartPolicy, ExitCodes
};
use std::fmt;

#[derive(Debug)]
pub enum ConfigValidationError {
	NoCommandGiven,
	ConfigInvalid(String)
}

#[derive(Debug, Deserialize)]
pub struct Config {
	pub program: HashMap<String, ProgramConfig>
}

impl fmt::Display for ConfigValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NoCommandGiven => write!(f, "No valid Command was given"),
			Self::ConfigInvalid(reason) => write!(f, "Config validation error: {reason}")
		}
	}
}

#[derive(Debug, Deserialize)]
pub struct ProgramConfig {
	pub command: Command,
	pub chdir: WorkingDir,
	pub chmod: Option<Umask>,
	pub process_count: ProcessCount,
	pub auto_start: bool,
	pub restart_protocol: RestartPolicy,
	pub exit_codes : ExitCodes,
	pub runtime: u32,
	pub restart_count: u32,
	pub exit_signal: String,
	pub kill_time: u32,
	pub stdin: Stdin,
	pub stdout: Stdout,
	pub env: Vec<String>
}

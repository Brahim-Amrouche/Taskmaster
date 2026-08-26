use std::collections::HashMap;

use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct Config {
	pub program: HashMap<String, ProgramConfig>
}

#[derive(Debug, Deserialize)]
pub struct ProgramConfig {
	pub command: String,
	pub chdir: String,
	pub chmod: String,
	pub process_count: u64,
	pub auto_start: bool,
	pub restart_protocol: Vec<String>,
	pub exit_codes : Vec<i32>,
	pub runtime: u32,
	pub restart_count: u32,
	pub exit_signal: String,
	pub kill_time: u32,
	pub stdin: String,
	pub stdout: String,
	pub env: Vec<String>
}


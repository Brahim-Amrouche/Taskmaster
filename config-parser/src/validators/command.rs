use std::{fs, path::PathBuf};
use serde::{Deserialize};
use std::os::unix::fs::PermissionsExt;

#[derive(Deserialize, Debug)]
#[serde(try_from="String")]
pub struct Command {
	pub path: PathBuf,
	pub args: Vec<String>
}

pub(crate) fn validate_command(command_path: &str) -> Result<PathBuf, String>{
	let command = PathBuf::from(command_path);
	if command.as_os_str().is_empty(){
		return Err("Command can't be empty".to_string());
	}
	if !command.exists() {
		return Err(format!("Command does not exist: {}", command.display()));
	}
	if !command.is_file(){
		return  Err(format!("Command is not a file: {}", command.display()));
	}
	let metadata = fs::metadata(&command)
		.map_err(|err| format!("cannot inspect command chmod: {err}"))?;

	let mode = metadata.permissions().mode();

	if mode & 0o111 == 0 {
		return Err(format!("Command is not executable: {}", command.display()));
	}
	return Ok(command);
}

pub(crate) fn validate_command_parts(command_string: &str) -> Result<(PathBuf, Vec<String>), String>{
	let mut raw_parts = command_string.trim().split_whitespace();
	let executable = raw_parts.next().ok_or_else(|| "Command cannot be empty".to_string())?;
	let command = validate_command(executable.trim())?;
	let args = raw_parts.map(String::from).collect();
	return Ok((command, args));
}

impl TryFrom<String> for Command {
	type Error = String;


	fn try_from(value: String) -> Result<Self, Self::Error> {
		let (path, args) = validate_command_parts(&value)?;
		return Ok(Self {path, args});
	}
}

#[cfg(test)]
mod tests {
	use super::{validate_command, validate_command_parts, Command};
	use std::fs;
	use std::os::unix::fs::PermissionsExt;
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	fn temporary_path(name: &str) -> PathBuf {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock must be after the Unix epoch")
			.as_nanos();

		std::env::temp_dir().join(format!(
			"taskmaster-command-{name}-{}-{timestamp}",
			std::process::id()
		))
	}

	#[test]
	fn rejects_an_empty_command() {
		let error = validate_command_parts("   ").expect_err("empty commands must fail");

		assert_eq!(error, "Command cannot be empty");
	}

	#[test]
	fn rejects_a_missing_executable() {
		let path = temporary_path("missing");
		let error = validate_command(path.to_str().expect("temporary path is UTF-8"))
			.expect_err("missing executables must fail");

		assert!(error.contains("Command does not exist"));
	}

	#[test]
	fn rejects_a_directory_as_an_executable() {
		let error = validate_command("/tmp").expect_err("directories are not executables");

		assert!(error.contains("Command is not a file"));
	}

	#[test]
	fn rejects_a_file_without_an_execute_bit() {
		let path = temporary_path("not-executable");
		fs::write(&path, "test file").expect("temporary file must be created");
		fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
			.expect("temporary file permissions must be updated");

		let result = validate_command(path.to_str().expect("temporary path is UTF-8"));
		fs::remove_file(&path).expect("temporary file must be removed");

		let error = result.expect_err("files without execute bits must fail");
		assert!(error.contains("Command is not executable"));
	}

	#[test]
	fn splits_an_executable_and_arguments() {
		let (path, arguments) = validate_command_parts("/usr/bin/ls -la /tmp")
			.expect("a valid executable with arguments must parse");

		assert_eq!(path, PathBuf::from("/usr/bin/ls"));
		assert_eq!(arguments, vec!["-la", "/tmp"]);
	}

	#[test]
	fn command_try_from_stores_the_parsed_command() {
		let command = Command::try_from("/usr/bin/ls -l".to_string())
			.expect("a valid command must deserialize");

		assert_eq!(command.path, PathBuf::from("/usr/bin/ls"));
		assert_eq!(command.args, vec!["-l"]);
	}
}



use serde::{Deserialize};
use std::path::{PathBuf};
use std::marker::PhantomData;

const DEFAULT_TARGET_OUTPUT: &str = "./stdout.log";

pub trait OutputTargetsType {
	fn name() ->  &'static str;
	fn validate(path: &mut PathBuf) -> Result<(), String>;
}

#[derive(Debug, Deserialize)]
#[serde(try_from="String")]
pub struct OutputTarget<T> where T: OutputTargetsType{
	pub target: PathBuf,
	_kind: PhantomData<T>
}

impl<T: OutputTargetsType>  TryFrom<String> for OutputTarget<T> {
	type Error = String;
	
	fn try_from(value: String) -> Result<OutputTarget<T>, Self::Error> {
		let mut target = match &value {
			v if v.len() > 0  => PathBuf::from(v),
			_ => PathBuf::from(DEFAULT_TARGET_OUTPUT)
		};
		T::validate(&mut target)?;
		return  Ok(OutputTarget {target, _kind: PhantomData});
	}
}

#[derive(Debug)]
pub struct StdoutOutputTarget;
#[derive(Debug)]
pub struct StdinOutputTarget;

pub type Stdin = OutputTarget<StdinOutputTarget>;
pub type Stdout = OutputTarget<StdoutOutputTarget>;

impl OutputTargetsType for StdinOutputTarget {
	fn name() ->  &'static str {
		"StdinOutputTarget"
	}

	fn validate(path: &mut PathBuf) -> Result<(), String> {
		if !path.is_file(){
			return Err("Stdin should be a valid file".into());
		}
		return  Ok(());
	}
}

impl OutputTargetsType for StdoutOutputTarget  {
	fn name() ->  &'static str {
		"StdoutOutputTarget"
	}
	
	fn validate(path: &mut PathBuf) -> Result<(), String> {
		if path.is_dir(){
			path.push("stdout.log");
		}
		return Ok(());
	}
}

#[cfg(test)]
mod tests {
	use super::{Stdin, Stdout};
	use std::fs;
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	fn temporary_path(name: &str) -> PathBuf {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock must be after the Unix epoch")
			.as_nanos();

		std::env::temp_dir().join(format!(
			"taskmaster-output-{name}-{}-{timestamp}",
			std::process::id()
		))
	}

	#[test]
	fn stdin_requires_an_existing_regular_file() {
		let path = temporary_path("stdin");
		fs::write(&path, "input").expect("temporary input file must be created");

		let result = Stdin::try_from(path.to_string_lossy().into_owned());
		fs::remove_file(&path).expect("temporary input file must be removed");

		let target = result.expect("an existing file is a valid stdin target");
		assert_eq!(target.target, path);
	}

	#[test]
	fn stdin_rejects_a_missing_file() {
		let path = temporary_path("missing-stdin");
		let error = Stdin::try_from(path.to_string_lossy().into_owned())
			.expect_err("a missing stdin file must fail");

		assert_eq!(error, "Stdin should be a valid file");
	}

	#[test]
	fn stdin_rejects_a_directory() {
		let error = Stdin::try_from("/tmp".to_string())
			.expect_err("directories are not valid stdin files");

		assert_eq!(error, "Stdin should be a valid file");
	}

	#[test]
	fn stdout_appends_a_filename_when_given_a_directory() {
		let target = Stdout::try_from("/tmp".to_string())
			.expect("a directory is a valid stdout target");

		assert_eq!(target.target, PathBuf::from("/tmp/stdout.log"));
	}

	#[test]
	fn stdout_accepts_a_nonexistent_output_path() {
		let path = temporary_path("stdout");
		let target = Stdout::try_from(path.to_string_lossy().into_owned())
			.expect("stdout currently accepts paths that do not exist yet");

		assert_eq!(target.target, path);
	}
}

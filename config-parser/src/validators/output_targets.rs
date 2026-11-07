
use serde::{Deserialize};
use std::path::{PathBuf};
use std::marker::PhantomData;
use std::fs::{File, OpenOptions};

pub trait OutputTargetsType {
	fn name() ->  &'static str;
	fn default_target() -> &'static str;
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
			_ => PathBuf::from(T::default_target())
		};
		T::validate(&mut target)?;
		return  Ok(OutputTarget {target, _kind: PhantomData});
	}
}

impl<T:OutputTargetsType> OutputTarget<T> {
	pub fn open_stream(&self) -> std::io::Result<File>{
		OpenOptions::new()
			.write(true)
			.truncate(true)
			.create(true)
			.open(&self.target)
	}
}


#[derive(Debug)]
pub struct StdoutOutputTarget;
#[derive(Debug)]
pub struct StderrOutputTarget;

pub type Stdout = OutputTarget<StdoutOutputTarget>;
pub type Stderr = OutputTarget<StderrOutputTarget>;

impl OutputTargetsType for StderrOutputTarget {
	fn name() ->  &'static str {
		"StderrOutputTarget"
	}

	fn default_target() -> &'static str {
		"./stderr.log"
	}

	fn validate(path: &mut PathBuf) -> Result<(), String> {
		if path.is_dir(){
			path.push("stderr.log");
		}
		return  Ok(());
	}
}


impl OutputTargetsType for StdoutOutputTarget  {
	fn name() ->  &'static str {
		"StdoutOutputTarget"
	}

	fn default_target() -> &'static str {
		"./stdout.log"
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
	use super::{Stderr, Stdout};
	use std::fs;
	use std::io::Write;
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

	#[test]
	fn stderr_appends_a_filename_when_given_a_directory() {
		let target = Stderr::try_from("/tmp".to_string())
			.expect("a directory is a valid stderr target");

		assert_eq!(target.target, PathBuf::from("/tmp/stderr.log"));
	}

	#[test]
	fn stderr_uses_its_own_default_file() {
		let target = Stderr::try_from(String::new())
			.expect("an empty stderr target must use the default file");

		assert_eq!(target.target, PathBuf::from("./stderr.log"));
	}

	#[test]
	fn opening_an_output_stream_truncates_an_existing_file() {
		let path = temporary_path("truncate");
		fs::write(&path, "old output that must be removed")
			.expect("existing output must be written");
		let target = Stdout::try_from(path.to_string_lossy().into_owned())
			.expect("output path must be valid");

		let mut stream = target.open_stream().expect("output stream must open");
		stream.write_all(b"new output").expect("new output must be written");
		drop(stream);

		assert_eq!(
			fs::read_to_string(&path).expect("truncated output must be readable"),
			"new output"
		);
		fs::remove_file(&path).expect("temporary output must be removed");
	}
}

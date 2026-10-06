use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::fmt;

pub struct ConfigReader {
	file_path: PathBuf,
	file_content: Option<String>,
}

#[derive(Debug)]
pub enum ConfigReaderError{
	FileNotReadable(String),
	NotLoaded,
	ParseError(String)
}

impl fmt::Display for ConfigReaderError {
	fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::FileNotReadable(file_path) => write!(f, "Couldn't read file `{file_path}`"),
			Self::NotLoaded => write!(f, "Nof file content is loaded"),
			Self::ParseError(err) => write!(f, "Failed to parse YAML configuration: {err}")
		}
	}
}

impl Error for ConfigReaderError {}

type ConfigReaderResult<T> = Result<T, ConfigReaderError>;

impl ConfigReader {
	pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
		Self { 
			file_path: file_path.as_ref().to_path_buf(),
			file_content: None 
		}
	}

	pub fn read(&mut self) -> ConfigReaderResult<()> {
		match fs::read_to_string(&self.file_path){
			Ok(content) => {
				self.file_content = Some(content);
				Ok(())
			}
			Err(_) => Err(ConfigReaderError::FileNotReadable(self.file_path.display().to_string()))
		}
	}

	pub fn parse<T>(&mut self) -> ConfigReaderResult<T> where T:DeserializeOwned {
		let content = self.file_content.as_ref().ok_or(ConfigReaderError::NotLoaded)?;
		toml::from_str::<T>(content).map_err(|err| ConfigReaderError::ParseError(err.to_string()))
	}
	
}

#[cfg(test)]
mod tests {
	use super::{ConfigReader, ConfigReaderError};
	use serde::Deserialize;
	use std::fs;
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	#[derive(Debug, Deserialize, PartialEq)]
	struct TestConfig {
		value: u32,
	}

	fn temporary_path(name: &str) -> PathBuf {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock must be after the Unix epoch")
			.as_nanos();

		std::env::temp_dir().join(format!(
			"taskmaster-config-reader-{name}-{}-{timestamp}.toml",
			std::process::id()
		))
	}

	#[test]
	fn parse_requires_the_file_to_be_read_first() {
		let mut reader = ConfigReader::new("unused.toml");
		let error = reader
			.parse::<TestConfig>()
			.expect_err("parsing before reading must fail");

		assert!(matches!(error, ConfigReaderError::NotLoaded));
	}

	#[test]
	fn read_reports_a_missing_file() {
		let path = temporary_path("missing");
		let mut reader = ConfigReader::new(&path);
		let error = reader.read().expect_err("missing files must fail to read");

		assert!(matches!(error, ConfigReaderError::FileNotReadable(_)));
	}

	#[test]
	fn reads_and_parses_valid_toml() {
		let path = temporary_path("valid");
		fs::write(&path, "value = 42").expect("temporary configuration must be created");

		let mut reader = ConfigReader::new(&path);
		let result = reader.read().and_then(|()| reader.parse::<TestConfig>());
		fs::remove_file(&path).expect("temporary configuration must be removed");

		assert_eq!(result.expect("valid TOML must parse"), TestConfig { value: 42 });
	}

	#[test]
	fn reports_invalid_toml_as_a_parse_error() {
		let path = temporary_path("invalid");
		fs::write(&path, "value = not-a-number")
			.expect("temporary invalid configuration must be created");

		let mut reader = ConfigReader::new(&path);
		reader.read().expect("temporary configuration must be readable");
		let result = reader.parse::<TestConfig>();
		fs::remove_file(&path).expect("temporary configuration must be removed");

		assert!(matches!(result, Err(ConfigReaderError::ParseError(_))));
	}
}

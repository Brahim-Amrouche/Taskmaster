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
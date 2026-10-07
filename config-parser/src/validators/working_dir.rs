use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
#[serde(try_from="String")]
pub struct WorkingDir {
	pub path: PathBuf
}

impl TryFrom<String> for WorkingDir {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		let path = PathBuf::from(value);
		if path.as_os_str().is_empty() {
			return Err("Working directory cannot be empty".to_string());
		}
		let metadata = fs::metadata(&path)
			.map_err(|e| format!("Cannot inspect working directory `{}`: {e}",path.display()))?;

		if !metadata.is_dir(){
			return Err(format!("Working directory is not a directory {}", path.display()));
		}
		Ok(Self { path })
	}
}

#[cfg(test)]
mod tests {
	use super::WorkingDir;
	use std::fs;
	use std::path::PathBuf;
	use std::time::{SystemTime, UNIX_EPOCH};

	fn temporary_path(name: &str) -> PathBuf {
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock must be after the Unix epoch")
			.as_nanos();

		std::env::temp_dir().join(format!(
			"taskmaster-working-dir-{name}-{}-{timestamp}",
			std::process::id()
		))
	}

	#[test]
	fn accepts_an_existing_directory() {
		let directory = WorkingDir::try_from("/tmp".to_string())
			.expect("an existing directory must be accepted");

		assert_eq!(directory.path, PathBuf::from("/tmp"));
	}

	#[test]
	fn rejects_an_empty_directory_path() {
		let error = WorkingDir::try_from(String::new())
			.expect_err("an empty directory path must fail");

		assert_eq!(error, "Working directory cannot be empty");
	}

	#[test]
	fn rejects_a_whitespace_only_directory_path() {
		assert!(WorkingDir::try_from("   ".to_string()).is_err());
	}

	#[test]
	fn rejects_a_missing_directory() {
		let path = temporary_path("missing");
		let error = WorkingDir::try_from(path.to_string_lossy().into_owned())
			.expect_err("a missing directory must fail");

		assert!(error.contains("Cannot inspect working directory"));
	}

	#[test]
	fn rejects_a_regular_file() {
		let path = temporary_path("file");
		fs::write(&path, "not a directory").expect("temporary file must be created");

		let result = WorkingDir::try_from(path.to_string_lossy().into_owned());
		fs::remove_file(&path).expect("temporary file must be removed");

		let error = result.expect_err("a regular file must not be accepted as a directory");
		assert!(error.contains("Working directory is not a directory"));
	}
}

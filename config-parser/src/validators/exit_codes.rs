use serde::{Deserialize};

#[derive(Debug,Deserialize)]
#[serde(try_from="Vec<u8>")]
pub struct ExitCodes {
	pub values: Vec<u8>
}

impl TryFrom<Vec<u8>> for ExitCodes {
	type Error = String;

	fn try_from(values: Vec<u8>) -> Result<Self, Self::Error> {
		if values.is_empty(){
			return Err("Exit codes cannot be empty".to_string());
		}
		let mut exit_codes = Vec::with_capacity(values.len());
		for value in values.iter() {
			if exit_codes.contains(value) {
				return Err(format!("Exit code is duplicated: {value}"));
			}
			exit_codes.push(*value);
		}
		return  Ok(Self { values: exit_codes });
	}
}

#[cfg(test)]
mod tests {
	use super::ExitCodes;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct TestConfig {
		exit_codes: ExitCodes,
	}

	#[test]
	fn accepts_unique_exit_codes() {
		let exit_codes = ExitCodes::try_from(vec![0, 2, 42])
			.expect("unique exit codes must be accepted");

		assert_eq!(exit_codes.values, vec![0, 2, 42]);
	}

	#[test]
	fn rejects_an_empty_exit_code_list() {
		let error = ExitCodes::try_from(Vec::new())
			.expect_err("an empty exit-code list must fail");

		assert_eq!(error, "Exit codes cannot be empty");
	}

	#[test]
	fn rejects_duplicate_exit_codes() {
		let error = ExitCodes::try_from(vec![0, 2, 0])
			.expect_err("duplicate exit codes must fail");

		assert_eq!(error, "Exit code is duplicated: 0");
	}

	#[test]
	fn serde_deserializes_valid_exit_codes() {
		let config: TestConfig = toml::from_str("exit_codes = [0, 2]")
			.expect("valid exit codes must deserialize");

		assert_eq!(config.exit_codes.values, vec![0, 2]);
	}

	#[test]
	fn serde_rejects_exit_codes_outside_the_u8_range() {
		for value in ["exit_codes = [-1]", "exit_codes = [256]"] {
			assert!(toml::from_str::<TestConfig>(value).is_err());
		}
	}
}

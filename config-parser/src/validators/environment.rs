use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(try_from = "HashMap<String, String>")]
pub struct EnvironmentVariables {
	pub values: HashMap<String, String>,
}

fn validate_name(name: &str) -> Result<(), String> {
	let mut characters = name.bytes();
	let first = characters
		.next()
		.ok_or_else(|| "Environment variable name cannot be empty".to_string())?;

	if !(first.is_ascii_alphabetic() || first == b'_') {
		return Err(format!(
			"Environment variable name must start with an ASCII letter or underscore: {name}"
		));
	}

	if !characters.all(|character| character.is_ascii_alphanumeric() || character == b'_') {
		return Err(format!(
			"Environment variable name must contain only ASCII letters, digits, or underscores: {name}"
		));
	}

	Ok(())
}

impl TryFrom<HashMap<String, String>> for EnvironmentVariables {
	type Error = String;

	fn try_from(values: HashMap<String, String>) -> Result<Self, Self::Error> {
		for name in values.keys() {
			validate_name(name)?;
		}

		Ok(Self { values })
	}
}

#[cfg(test)]
mod tests {
	use super::EnvironmentVariables;
	use serde::Deserialize;
	use std::collections::HashMap;

	#[derive(Deserialize)]
	struct TestConfig {
		env: EnvironmentVariables,
	}

	#[test]
	fn accepts_posix_style_environment_variable_names() {
		let environment = EnvironmentVariables::try_from(HashMap::from([
			("MODE".to_string(), "production".to_string()),
			("_WORKER_2".to_string(), "enabled".to_string()),
		]))
		.expect("valid environment names must be accepted");

		assert_eq!(environment.values.len(), 2);
		assert_eq!(environment.values.get("MODE"), Some(&"production".to_string()));
	}

	#[test]
	fn rejects_an_empty_environment_variable_name() {
		let error = EnvironmentVariables::try_from(HashMap::from([(
			String::new(),
			"value".to_string(),
		)]))
		.expect_err("an empty environment name must be rejected");

		assert_eq!(error, "Environment variable name cannot be empty");
	}

	#[test]
	fn rejects_names_that_start_with_invalid_characters() {
		for name in ["1MODE", "-MODE", "=MODE"] {
			let error = EnvironmentVariables::try_from(HashMap::from([(
				name.to_string(),
				"value".to_string(),
			)]))
			.expect_err("invalid leading characters must be rejected");

			assert!(error.contains("must start"));
		}
	}

	#[test]
	fn rejects_names_with_invalid_characters() {
		for name in ["MODE-NAME", "MODE=NAME", "MODE.NAME"] {
			let error = EnvironmentVariables::try_from(HashMap::from([(
				name.to_string(),
				"value".to_string(),
			)]))
			.expect_err("invalid characters must be rejected");

			assert!(error.contains("must contain"));
		}
	}

	#[test]
	fn serde_rejects_invalid_environment_names() {
		let result = toml::from_str::<TestConfig>("env = { \"1MODE\" = \"value\" }");

		assert!(result.is_err());
	}

	#[test]
	fn serde_accepts_valid_environment_names_and_arbitrary_values() {
		let configuration: TestConfig = toml::from_str(
			"env = { MODE = \"production mode\", _WORKER_2 = \"value=with=equals\" }",
		)
		.expect("valid environment map must deserialize");

		assert_eq!(
			configuration.env.values.get("_WORKER_2"),
			Some(&"value=with=equals".to_string())
		);
	}
}

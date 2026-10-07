use serde::{Deserialize};


#[derive(Deserialize, Debug)]
#[serde(try_from="u64")]
pub struct ProcessCount{
	pub value: u64
}

impl TryFrom<u64> for ProcessCount {
	type Error = String;

	fn try_from(value: u64) -> Result<Self, Self::Error> {
		if value == 0{
			return Err("Process count must be greater than zero".to_string());
		}
		Ok(Self { value })
	}
}

#[cfg(test)]
mod tests {
	use super::ProcessCount;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct TestConfig {
		process_count: ProcessCount,
	}

	#[test]
	fn accepts_a_positive_process_count() {
		let count = ProcessCount::try_from(3)
			.expect("a positive process count must be accepted");

		assert_eq!(count.value, 3);
	}

	#[test]
	fn rejects_zero_processes() {
		let error = ProcessCount::try_from(0)
			.expect_err("zero processes must be rejected");

		assert_eq!(error, "Process count must be greater than zero");
	}

	#[test]
	fn serde_deserializes_a_valid_process_count() {
		let config: TestConfig = toml::from_str("process_count = 2")
			.expect("a positive process count must deserialize");

		assert_eq!(config.process_count.value, 2);
	}

	#[test]
	fn serde_rejects_zero_process_count() {
		let result = toml::from_str::<TestConfig>("process_count = 0");

		assert!(result.is_err());
	}
}



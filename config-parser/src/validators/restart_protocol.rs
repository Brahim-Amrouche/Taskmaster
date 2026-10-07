use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum  RestartPolicy {
	Always,
	Never,
	Unexpected
}

#[cfg(test)]
mod tests {
	use super::RestartPolicy;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct TestConfig {
		restart_policy: RestartPolicy,
	}

	#[test]
	fn serde_deserializes_each_valid_restart_policy() {
		let always: TestConfig = toml::from_str("restart_policy = \"always\"")
			.expect("always must deserialize");
		let never: TestConfig = toml::from_str("restart_policy = \"never\"")
			.expect("never must deserialize");
		let unexpected: TestConfig = toml::from_str("restart_policy = \"unexpected\"")
			.expect("unexpected must deserialize");

		assert!(matches!(always.restart_policy, RestartPolicy::Always));
		assert!(matches!(never.restart_policy, RestartPolicy::Never));
		assert!(matches!(unexpected.restart_policy, RestartPolicy::Unexpected));
	}

	#[test]
	fn serde_rejects_an_unknown_restart_policy() {
		let result = toml::from_str::<TestConfig>("restart_policy = \"sometimes\"");

		assert!(result.is_err());
	}
}

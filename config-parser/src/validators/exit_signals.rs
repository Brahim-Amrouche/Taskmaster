use serde::{Deserialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExitSignals {
	Term,
	Kill,
	Int,
	Hup,
	Quit,
}

#[cfg(test)]
mod tests {
	use super::ExitSignals;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct TestConfig {
		exit_signal: ExitSignals,
	}

	#[test]
	fn serde_deserializes_each_supported_signal() {
		let term: TestConfig = toml::from_str("exit_signal = \"TERM\"")
			.expect("TERM must deserialize");
		let kill: TestConfig = toml::from_str("exit_signal = \"KILL\"")
			.expect("KILL must deserialize");
		let int: TestConfig = toml::from_str("exit_signal = \"INT\"")
			.expect("INT must deserialize");
		let hup: TestConfig = toml::from_str("exit_signal = \"HUP\"")
			.expect("HUP must deserialize");
		let quit: TestConfig = toml::from_str("exit_signal = \"QUIT\"")
			.expect("QUIT must deserialize");

		assert!(matches!(term.exit_signal, ExitSignals::Term));
		assert!(matches!(kill.exit_signal, ExitSignals::Kill));
		assert!(matches!(int.exit_signal, ExitSignals::Int));
		assert!(matches!(hup.exit_signal, ExitSignals::Hup));
		assert!(matches!(quit.exit_signal, ExitSignals::Quit));
	}

	#[test]
	fn serde_rejects_unsupported_or_lowercase_signals() {
		for signal in ["STOP", "term"] {
			let configuration = format!("exit_signal = \"{signal}\"");
			assert!(toml::from_str::<TestConfig>(&configuration).is_err());
		}
	}
}

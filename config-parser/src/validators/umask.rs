use serde::{Deserialize};

#[derive(Deserialize, Debug)]
#[serde(try_from="String")]
pub struct Umask {
	pub value: u16
}

impl TryFrom<String> for Umask {
	type Error = String;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		let value = value.trim();

		if value.len() != 3 {
			return  Err("Umask must contain exactly three octal digits".to_string());
		}
		if !value.bytes().all(|byte| matches!(byte, b'0'..=b'7')) {
			return Err("umask must contain only octal digits from 0 to 7".to_string());
		}
		let value = u16::from_str_radix(value, 8)
		.map_err(|err| format!("invalid umask: {err}"))?;
		Ok( Self { value })
	}
}

#[cfg(test)]
mod tests {
	use super::Umask;
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct TestConfig {
		umask: Umask,
	}

	#[test]
	fn accepts_valid_octal_umasks() {
		assert_eq!(Umask::try_from("000".to_string()).unwrap().value, 0o000);
		assert_eq!(Umask::try_from("022".to_string()).unwrap().value, 0o022);
		assert_eq!(Umask::try_from("777".to_string()).unwrap().value, 0o777);
	}

	#[test]
	fn trims_surrounding_whitespace() {
		let umask = Umask::try_from(" 022 \n".to_string())
			.expect("surrounding whitespace must be ignored");

		assert_eq!(umask.value, 0o022);
	}

	#[test]
	fn rejects_umasks_with_the_wrong_number_of_digits() {
		for value in ["", "22", "0022"] {
			assert!(Umask::try_from(value.to_string()).is_err());
		}
	}

	#[test]
	fn rejects_non_octal_umasks() {
		for value in ["089", "abc"] {
			assert!(Umask::try_from(value.to_string()).is_err());
		}
	}

	#[test]
	fn serde_deserializes_a_valid_umask() {
		let config: TestConfig = toml::from_str("umask = \"077\"")
			.expect("a valid octal umask must deserialize");

		assert_eq!(config.umask.value, 0o077);
	}

	#[test]
	fn serde_rejects_an_invalid_umask() {
		let result = toml::from_str::<TestConfig>("umask = \"099\"");

		assert!(result.is_err());
	}
}

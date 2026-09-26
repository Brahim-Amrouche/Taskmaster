
use serde::{Deserialize};
use std::path::{PathBuf};
use std::marker::PhantomData;

const DEFAULT_TARGET_OUTPUT: &str = "./stdout.log";

pub trait OutputTargetsType {
	fn name() ->  &'static str;
	fn validate(path: &mut PathBuf) -> Result<(), String>;
}

#[derive(Debug, Deserialize)]
#[serde(try_from="String")]
pub struct OutputTarget<T> where T: OutputTargetsType{
	target: PathBuf,
	_kind: PhantomData<T>
}

impl<T: OutputTargetsType>  TryFrom<String> for OutputTarget<T> {
	type Error = String;
	
	fn try_from(value: String) -> Result<OutputTarget<T>, Self::Error> {
		let mut target = match &value {
			v if v.len() > 0  => PathBuf::from(v),
			_ => PathBuf::from(DEFAULT_TARGET_OUTPUT)
		};
		T::validate(&mut target)?;
		return  Ok(OutputTarget {target, _kind: PhantomData});
	}
}

#[derive(Debug)]
pub struct StdoutOutputTarget;
#[derive(Debug)]
pub struct StdinOutputTarget;

pub type Stdin = OutputTarget<StdinOutputTarget>;
pub type Stdout = OutputTarget<StdoutOutputTarget>;

impl OutputTargetsType for StdinOutputTarget {
	fn name() ->  &'static str {
		"StdinOutputTarget"
	}

	fn validate(path: &mut PathBuf) -> Result<(), String> {
		println!("{:?}", path);
		if !path.is_file(){
			return Err("Stdin should be a valid file".into());
		}
		return  Ok(());
	}
}

impl OutputTargetsType for StdoutOutputTarget  {
	fn name() ->  &'static str {
		"StdoutOutputTarget"
	}
	
	fn validate(path: &mut PathBuf) -> Result<(), String> {
		if path.is_dir(){
			path.push("stdout.log");
		}
		return Ok(());
	}
}

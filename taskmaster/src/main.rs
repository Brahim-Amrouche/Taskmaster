
use config_parser::config_reader::ConfigReader;

fn main() -> Result<(), Box<dyn std::error::Error> > {
	let mut config_reader = ConfigReader::new("./Cargo.toml");
	config_reader.read()?;
    println!("Hello, world!");
	Ok(())
}

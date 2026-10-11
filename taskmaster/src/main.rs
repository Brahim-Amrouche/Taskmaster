
use config_parser::config_reader::ConfigReader;
use config_parser::config::Config;
use runtime::process::RuntimeProcess;

fn main() -> Result<(), Box<dyn std::error::Error> > {
	if let Err(error) = run() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
	Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error> > {
	let mut config_reader = ConfigReader::new("./Run.toml");
	config_reader.read()?;
	let configs = config_reader.parse::<Config>()?;
	for (_, conf) in configs.program.into_iter(){
		let mut process = RuntimeProcess::new(&conf);
		process.run()?;
		// println!("The value for program is {:?}",conf.command);
	}

	println!("Hello, world!");
	Ok(())
}
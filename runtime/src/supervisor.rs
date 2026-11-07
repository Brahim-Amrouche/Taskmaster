
use crate::process::{RuntimeProcess};

pub struct Supervisor<'a> {
	processes: Vec<RuntimeProcess<'a>>,

}

impl<'a> Supervisor<'a> {
	pub fn new(processes: Vec<RuntimeProcess<'a>>) -> Self {

		
		Self {
			processes
		}
	}
}
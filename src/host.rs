use std::fs::File;
use server::{Launch, rocket};
use crate::database::frequency::Frequency;

const CORRF: &str = "results/frequency/corrindex";

pub struct Server<'a> {
	data: Frequency<'a>
}

impl Launch for Server<'_> {
	fn new() -> Server<'static> {
		Server {
			data: bincode::deserialize_from(
				File::open(CORRF).unwrap()
			).unwrap()
		}
	}

	fn mount(&self, fpath: &str, app: rocket::Rocket) -> rocket::Rocket {
		app
	}
}
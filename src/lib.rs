#![feature(proc_macro_hygiene, decl_macro)]

use std::path::{Path, PathBuf};
use std::fs::File;
use server::{
	Launch,
	rocket::{
		self,
		State,
		get, post,
		routes,
		response::content
	},
	rocket_contrib::{
		serve::StaticFiles,
		json::Json
	}
};
use serde::Deserialize;
use database::correlation::Correlation;

pub mod database;

const CORRF: &str = "results/frequency/corrindex.dat";

#[derive(Deserialize)]
enum Req {
	Test
}

pub struct Server {
	data: Correlation,
	static_f: StaticFiles
}

/// Get correlation data between two sets of words
#[post("/corr", format = "json", data = "<data>")]
fn corr (data: Json<Req>, state: State<Correlation>) -> content::Json<&'static str> {
	todo!();
}

impl Launch for Server {
	fn new <P: AsRef<Path>> (root: P) -> std::io::Result<Server> {
		let root = root.as_ref();

		Ok(Server {
			data: bincode::deserialize_from(
				File::open(root.join(CORRF))?
			).unwrap(),
			static_f: StaticFiles::from(root.join("static"))
		})
	}

	fn mount (self, path: &str, app: rocket::Rocket) -> rocket::Rocket {
		app
			.manage(self.data)
			.mount(path, routes![corr])
			.mount(path, self.static_f)
	}
}
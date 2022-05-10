#![feature(proc_macro_hygiene, decl_macro)]

use std::path::{Path, PathBuf};
use lru::LruCache;
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
use lazy_static::lazy_static;
use rand::{
	SeedableRng,
	rngs::SmallRng
};
use chrono::NaiveDate;

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

struct CState {
	cache: LruCache<u32, Vec<f64>>,
	corr: Correlation,
}

impl CState {
	pub fn new(corr: Correlation, sz: usize) -> CState {
		CState {
			cache: LruCache::new(sz),
			corr
		}
	}
}


lazy_static! {
	static ref ROOT_DATE : NaiveDate = NaiveDate::from_ymd(2022, 5, 9);
	static ref WORD_LIST : Vec<String> = Vec::new();


}

/// Get correlation data between two sets of words
#[post("/dev/corr", format = "json", data = "<data>")]
fn corr (data: Json<Req>, state: State<CState>) -> content::Json<&'static str> {
	todo!();
}

#[get("/guess?<word>")]
fn guess(word: String, state: State<CState>) -> content::Json<&'static str> {
	todo!();
}

impl Launch for Server {
	fn new <P: AsRef<Path>> (root: P) -> std::io::Result<Server> {
		let root = root.as_ref();

		Ok(Server {
			data: bincode::deserialize_from(
				File::open(root.join(CORRF))?
			).map_err(|_| std::io::ErrorKind::InvalidData)?,
			static_f: StaticFiles::from(root.join("static"))
		})
	}

	fn mount <P: AsRef<Path>> (self, path: P, app: rocket::Rocket) -> rocket::Rocket {
		let path = path.as_ref();

		const CACHE_LEN: usize = 1000;

		app
			.manage(CState::new(self.data, CACHE_LEN))
			.mount(path.join("api").to_str().unwrap_or("/api"), routes![corr])
			.mount(path.to_str().unwrap_or(""), self.static_f)
	}
}
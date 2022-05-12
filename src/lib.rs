#![feature(proc_macro_hygiene, decl_macro, try_trait_v2)]

use serde::{Serialize, Deserialize};
use std::sync::RwLock;
use std::borrow::BorrowMut;
use std::path::{Path};
use lru::LruCache;
use std::fs::File;
use server::{
	Launch,
	rocket::{
		self,
		State,
		get, post,
		routes,
		http::{Status,ContentType},
		response::{Response, Responder, content},
	},
	rocket_contrib::{
		serve::StaticFiles,
		json::{Json}
	}
};

use database::correlation::Correlation;
use std::io::{BufRead,BufReader,Cursor};
use lazy_static::lazy_static;
use rand::{
	SeedableRng,
	prelude::SliceRandom,
	rngs::SmallRng,
};
use chrono::{
	NaiveDate,
	offset::Utc,
};
use log::{debug, trace};

pub mod database;

const VALID_ANSWERS: &str = "data/answers"; // valid answer words
const CORRF: &str = "results/frequency/corrindex.dat";

const RNG_SEED: <SmallRng as SeedableRng>::Seed = *b"MERLIN 2.0 fan v.s. HMM enjoyer\n";

lazy_static! {
	static ref ROOT_DATE : NaiveDate = NaiveDate::from_ymd(2022, 5, 9);
}

struct CState {
	cache: LruCache<u32, Vec<f64>>,
	corr: Correlation,
	wordlist: Vec<String>,
}

impl CState {
	pub fn new <P: AsRef<Path>> (root: P, sz: usize) -> std::io::Result<CState> {
		let root = root.as_ref();

		let f = BufReader::new(File::open(VALID_ANSWERS)?);
		let mut rng = SmallRng::from_seed(RNG_SEED);

		let mut wordlist: Vec<String> = f
			.lines()
			.map(|e| e.unwrap())
			.collect();
		wordlist.shuffle(&mut rng);

		Ok (CState {
			cache: LruCache::new(sz),
			corr: bincode::deserialize_from(
					File::open(root.join(CORRF))?
				).map_err(|_| std::io::ErrorKind::InvalidData)?,
			wordlist,
		})
	}

	/// Current correct answer
	pub fn answer (&self) -> &String {
		&self.wordlist[Utc::today()
			.naive_utc()
			.signed_duration_since(*ROOT_DATE)
			.num_days()
			.rem_euclid(self.wordlist.len() as i64)
			as usize]
	}

	/// load correlation data for a word and cache it
	fn loadcorr	(&mut self, w: &str) -> Option<Vec<f64>> {
		let wind = self.corr.index(w)?;

		match self.cache.get(&wind) {
			Some(_) => (),
			None => {
				self.cache.push(wind, self.corr.corrall(w)?);
			}
		}

		Some(self.cache.peek(&wind)?.clone())
	}

	/// correlation between words `a` and `b`
	pub fn corr (&mut self, a: &str, b: &str) -> Option<f64> {
		let a = self.corr.index(a)?;

		Some(match self.cache.get(&a) {
			Some(c) => c[self.corr.index(b)? as usize],
			None => self.loadcorr(b)?[a as usize]
		})
	}
}

type MState = RwLock<CState>;

pub struct Server {
	data: MState,
	static_f: StaticFiles
}

fn reject (status: Status, msg: &str) -> Response {
	Response::build()
		.status(status)
		.header(ContentType::Plain)
		.streamed_body(msg.as_bytes())
		.finalize()
}

fn accept <T : Serialize> (data: T) -> Response<'static> {
	Response::build()
		.sized_body(
			Cursor::new(serde_json::to_vec(&data).unwrap())
		).finalize()
}

/// Get correlation data between two sets of words
#[post("/dev/corr", format = "json", data = "<data>")]
fn corr (data: Json<(Vec<String>, Vec<String>)>, mut state: State<MState>) -> Response {
	let (a, b) = data.into_inner();
	let mut state = match state.borrow_mut().write() {
		Err(_) => return reject(Status::BadRequest, "Could not access internal server data."),
		Ok(s) => s
	};

	match a.into_iter()
		.map(|i|
			b.iter()
				.map(|j| {
					state.corr(&i, j)
				}).collect()
		).collect::<Option<Vec<Vec<f64>>>>() {
		None => reject(Status::BadRequest, "Input requested data for invalid words"),
		Some (e) => accept(e)
	}
}

/// Returned when a guess is made
#[derive(Serialize,Debug)]
struct GuessData {
	rank: u32, // approximate rank
	corr: f64,
	correct: bool // is the word the answer for today?
}

/// Guess a word.
#[get("/guess?<word>")]
fn guess(word: String, state: State<MState>) -> Response {
	let mut state = match state.write() {
		Err(_) => return reject(Status::BadRequest, "Could not access internal state"),
		Ok(s) => s
	};

	let ans = state.answer().clone();

	let mut guess = || -> Option<GuessData> {
		Some(GuessData {
			corr: state.corr(&word, &ans)?,
			rank: 0,
			correct: word == ans
		})
	};

	match guess() {
		Some (k) => {
			debug!(target:"app::dump", "Guessed {}, correct word was {}, data: {:?}", word, ans, k);
			accept(k)
		},
		None => reject(Status::BadRequest, "Guessed an invalid word.")
	}
}

impl Launch for Server {
	fn new <P: AsRef<Path>> (root: P) -> std::io::Result<Server> {
		let root = root.as_ref();

		const CACHE_LEN: usize = 1000;

		Ok(Server {
			data: RwLock::new(CState::new(
				root,
				CACHE_LEN 
			)?),
			static_f: StaticFiles::from(root.join("static"))
		})
	}

	fn mount <P: AsRef<Path>> (self, path: P, app: rocket::Rocket) -> rocket::Rocket {
		let path = path.as_ref();

		app
			.manage(self.data)
			.mount(path.join("api").to_str().unwrap_or("api/"), routes![corr, guess])
			.mount(path.to_str().unwrap_or(""), self.static_f)
	}
}
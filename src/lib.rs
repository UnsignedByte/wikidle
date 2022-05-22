#![feature(proc_macro_hygiene, decl_macro, try_trait_v2)]

use std::collections::HashMap;
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
	ranks: LruCache<u32, HashMap<u32, usize>>,
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
			ranks: LruCache::new(10),
			corr: bincode::deserialize_from(
					File::open(root.join(CORRF))?
				).map_err(|_| std::io::ErrorKind::InvalidData)?,
			wordlist,
		})
	}

	/// All valid words
	pub fn words(&self) -> Vec<String> {
		self.corr.dict().keys()
			.map(|e| e.clone())
			.collect()
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
	fn corrs	(&mut self, w: &str) -> Option<Vec<f64>> {
		let wind = self.corr.index(w)?;

		match self.cache.get(&wind) {
			Some(_) => (),
			None => {
				self.cache.push(wind, self.corr.corrall(w)?);
			}
		}

		Some(self.cache.peek(&wind)?.clone())
	}

	/// get ranks for a word
	fn ranks(&mut self, w: &str) -> Option<HashMap<u32, usize>>{
		let wind = self.corr.index(w)?;

		match self.ranks.get(&wind) {
			Some(_) => (),
			None => {
				let dat = self.corrs(w)?;

				let mut dat: Vec<(usize,f64)> = dat.into_iter()
					.enumerate()
					.collect();

				dat.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

				let dat: HashMap<u32, usize> = dat.into_iter()
					.enumerate()
					.map(|(i, (w, _))| (w as u32, i))
					.collect();

				self.ranks.push(wind, dat);
			}
		}

		Some(self.ranks.peek(&wind)?.clone())
	}

	/// correlation between words `a` and `b`
	pub fn corr (&mut self, a: &str, b: &str) -> Option<f64> {
		let a = self.corr.index(a)?;

		Some(match self.cache.get(&a) {
			Some(c) => c[self.corr.index(b)? as usize],
			None => self.corrs(b)?[a as usize]
		})
	}

	/// get rank of word `b` in word `a`'s list
	pub fn rank (&mut self, a: &str, b: &str) -> Option<usize> {
		self.ranks(a)?
			.get(&self.corr.index(b)?)
			.map(|e| *e)
	}
}

type MState = RwLock<CState>;

pub struct Server {
	data: MState,
	static_f: StaticFiles
}

fn reject (status: Status, msg: &str) -> Response<'static> {
	Response::build()
		.status(status)
		.header(ContentType::Plain)
		.sized_body(
			Cursor::new(msg.to_owned().into_bytes())
		).finalize()
}

fn accept <T : Serialize> (data: T) -> Response<'static> {
	Response::build()
		.sized_body(
			Cursor::new(serde_json::to_vec(&data).unwrap())
		).finalize()
}

/// Get correlation data between two sets of words
#[post("/dev/corr", format = "json", data = "<data>")]
fn corr (data: Json<(Vec<String>, Vec<String>)>, state: State<MState>) -> Response {
	let (a, b) = data.into_inner();
	let mut state = match state.write() {
		Err(_) => return reject(Status::BadRequest, "Could not access internal state (Server error)."),
		Ok(s) => s
	};

	match a.into_iter()
		.map(|i|
			b.iter()
				.map(|j| {
					state.corr(&i, j)
				}).collect()
		).collect::<Option<Vec<Vec<f64>>>>() {
		None => reject(Status::BadRequest, "Some words were invalid."),
		Some (e) => accept(e)
	}
}

/// Get raw rank and corr data for a word
#[get("/dev/raw?<word>")]
fn raw (word: String, state: State<MState>) -> Response {
	let mut state = match state.write() {
		Err(_) => return reject(Status::BadRequest, "Could not access internal state (Server error)."),
		Ok(s) => s
	};

	let mut set : Vec<(usize, String)> = 
		match state.words()
			.into_iter()
			.map(|e| match state.rank(&word, &e) {
				Some (k) => Some ( (k, e) ),
				None => None
			})
			.collect::<Option<Vec<(usize, String)>>>() {
			None => return reject(Status::BadRequest, &format!("{word} was not a valid word.")),
			Some (k) => k
		};

	set.sort_unstable_by_key(|(a, _)| *a);

	accept(
		set.into_iter()
			.map(|(_,k)| (state.corr(&word, &k).unwrap(), k))
			.map(|(a,b)| (b,a))
			.collect::<Vec<(String,f64)>>()
	)
}

/// Returned when a guess is made
#[derive(Serialize,Debug)]
struct GuessData {
	rank: usize, // approximate rank
	corr: f64,
	correct: bool // is the word the answer for today?
}

/// Guess a word.
#[get("/guess?<word>")]
fn guess(word: String, state: State<MState>) -> Response {
	let mut state = match state.write() {
		Err(_) => return reject(Status::BadRequest, "Could not access internal state (Server error)."),
		Ok(s) => s
	};

	let ans = state.answer().clone();

	let mut guess = || -> Option<GuessData> {
		Some(GuessData {
			corr: state.corr(&ans, &word)?,
			rank: state.rank(&ans, &word)?,
			correct: word == ans
		})
	};

	match guess() {
		Some (k) => {
			debug!(target:"app::dump", "Guessed {}, correct word was {}, data: {:?}", word, ans, k);
			accept(k)
		},
		None => reject(Status::BadRequest, &format!("{word} was not a valid word."))
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
			.mount(path.join("api").to_str().unwrap_or("api/"), routes![corr, raw, guess])
			.mount(path.to_str().unwrap_or(""), self.static_f)
	}
}
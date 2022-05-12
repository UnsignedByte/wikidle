#![feature(async_closure)]

use core::time::Duration;
use futures::{stream,StreamExt};
use std::collections::HashSet;
use std::path::Path;
use server::{
	Launch,
	rocket::{
		self,
		config::{Config, Environment, LoggingLevel}
	}
};
use std::collections::HashMap;
use wikidle::{
	Server,
	database::{
		self,
		read::{Dict, load_dict, Database},
		correlation::Correlation,
		frequency::{Frequency}
	}
};
use std::io::{Write, BufWriter, BufReader, Seek, SeekFrom};
use bzip2::bufread::{MultiBzDecoder};
use std::fs::File;
use serde::{
	Deserialize,
	de::{self, Deserializer, Visitor, MapAccess, IgnoredAny}
};
use core::fmt::Formatter;
use const_format::formatcp;
use log::{info, error};

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);
const DBDICT: &str = "data/words";
const VALID_ANSWERS: &str = "data/answers"; // valid answer words
const DICT_URI: &str = "https://api.dictionaryapi.dev/api/v2/entries/en/";

/// enum representing a part of speech
#[derive(Debug, Hash, PartialEq, Eq)]
enum PartOfSpeech {
	Noun,
	Verb,
	Adjective,
	Adverb,
	Preposition,
	Conjunction
}

impl<'a> Deserialize<'a> for PartOfSpeech {
	fn deserialize<D>(deserializer: D) -> Result<PartOfSpeech, D::Error>
		where D: Deserializer<'a>,
	{
		// implementation following https://serde.rs/deserialize-struct.html

		struct WVisitor;

		impl<'de> Visitor<'de> for WVisitor {
			type Value = PartOfSpeech;

			fn expecting(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
				formatter.write_str("JSON word info.")
			}

			fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where V: MapAccess<'de>,
      {
      	let mut part = None;

      	while let Some(key) = map.next_key::<String>()? {
      		match key.to_lowercase().as_str() {
      			"partofspeech" => {
      				if part.is_some() {
      					return Err(de::Error::duplicate_field("partOfSpeech"));
      				}

      				let v = map.next_value::<String>()?;

      				part = Some(match v.to_lowercase().as_str() {
      					"noun" => PartOfSpeech::Noun,
      					"verb" => PartOfSpeech::Verb,
      					"adjective" => PartOfSpeech::Adjective,
      					"adverb" => PartOfSpeech::Adverb,
      					"preposition" => PartOfSpeech::Preposition,
      					"conjunction" => PartOfSpeech::Conjunction,
      					s => return Err(de::Error::invalid_value(de::Unexpected::Str(&s), &"part of speech"))
      				});
      			},
      			_ => {
      				let _ = map.next_value::<IgnoredAny>()?;
      			}
      		}
      	}

				Ok(part.ok_or_else(|| de::Error::missing_field("partOfSpeech"))?)
      }
		}

    const FIELDS: &'static [&'static str] = &["word", "parts"];
		deserializer.deserialize_struct("WordReq", FIELDS, WVisitor)
	}
}

/// struct representing the return of a request to the free dictionary api
#[derive(Debug)]
struct WordReq {
	word: String,
	parts: HashSet<PartOfSpeech>,
}

impl<'a> Deserialize<'a> for WordReq {
	fn deserialize<D>(deserializer: D) -> Result<WordReq, D::Error>
		where D: Deserializer<'a>,
	{
		// implementation following https://serde.rs/deserialize-struct.html

		struct WVisitor;

		impl<'de> Visitor<'de> for WVisitor {
			type Value = WordReq;

			fn expecting(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
				formatter.write_str("JSON word info.")
			}

			fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where V: MapAccess<'de>,
      {
      	let mut word = None;
      	let mut parts = None;

      	while let Some(key) = map.next_key::<String>()? {
      		match key.to_lowercase().as_str() {
      			"word" => {
      				if word.is_some() {
      					return Err(de::Error::duplicate_field("word"));
      				}

      				word = Some(map.next_value()?);
      			},
      			"meanings" => {
      				if parts.is_some() {
      					return Err(de::Error::duplicate_field("parts"));
      				}

      				parts = Some(map.next_value()?);
      			},
      			_ => {
      				let _ = map.next_value::<IgnoredAny>()?;
      			}
      		}
      	}

      	let word: String = word.ok_or_else(|| de::Error::missing_field("word"))?;
      	let parts: HashSet<PartOfSpeech> = parts.ok_or_else(|| de::Error::missing_field("parts"))?;

				Ok(WordReq {word, parts})
      }
		}

    const FIELDS: &'static [&'static str] = &["word", "parts"];
		deserializer.deserialize_struct("WordReq", FIELDS, WVisitor)
	}
}

/// Get set of parts of speech of word
async fn word_parts (word: &str) -> Option<HashSet<PartOfSpeech>> {
	let req = loop {
		let req = reqwest::get(format!("{}{}", DICT_URI, word))
			.await.ok()?;

		match req.status().as_u16() {
			200 => break req,
			429 => (),
			_ => return None
		}

		info!("{}: Failed fetch, trying again in 0.5 seconds.", word);

		tokio::time::sleep(Duration::from_millis(1000)).await;
	};

	Some(
			req
			.json::<Vec<WordReq>>()
			.await.ok()?
			.into_iter()
			.fold(HashSet::new(), |acc, e| {
				if word == e.word {
					acc.into_iter()
						.chain(e.parts.into_iter())
						.collect()
				} else {
					acc
				}
			})
	)
}

async fn gen_word_frequency<'a> (namespace: &str, dict: &'a Dict, start: u64) {
	let path = Path::new("results").join(namespace);

	let root = path.join("index.dat");
	let cind = path.join("corrindex.dat");
	let cpath = path.join("corr.dat");

	let valid = File::open(VALID_ANSWERS);
	let cexist = File::open(&cind);

	if let (Ok(_), Ok(_)) = (&valid, &cexist) {
		info!("Correlation data already exists, returning.");
		return;
	}

	let mut fa = match File::open(&root) {
		Ok(f) => {
			info!("Loading database.");
			match bincode::deserialize_from(f) {
				Ok(d) => d,
				Err(e) => panic!("Failed to deserialize database with error:\n{e}")
			}
		}
		Err(_) => {
			info!("Failed to read database, creating new instead.");

			let mut db = File::open(format!("{}.bz2", DBDATA)).unwrap();

			db.seek(SeekFrom::Start(start)).unwrap();

			let db = BufReader::new(db);
			let db = MultiBzDecoder::new(db);
			// let db = File::open(DBDATA).unwrap();
			let db = BufReader::new(db);
			let db = Database::new(db);

			let mut a = db.into_iter();

			std::fs::create_dir_all(&path).unwrap();

			let mut fa = Frequency::new(path.join("data.dat"), &dict).unwrap();

			while let Some(e) = a.next() {
				let page = match e {
					Ok(x) => x,
					Err(x) => {
						error!("Failed to parse article with error {x:?}, skipped");
						continue;
					}
				};
				fa.insert(page.text).unwrap();

				info!(target: "app::basic", "Parsed article {}: {}", page.id, page.title);
			}

			let fw = BufWriter::new(File::create(&root).unwrap());

			bincode::serialize_into(fw, &fa).unwrap();

			fa
		}
	};

	info!("Loading freq data to memory");
	let dat = fa.load().unwrap();

	if let Err(_) = valid {
		let mut f = BufWriter::new(File::create(VALID_ANSWERS).unwrap());

		let words: HashMap<u32, u32> = dat.iter()
			.map(|(k, v)| 
				(*k, v.into_iter().map(|(_, v)| *v as u32).sum())
			).collect();

		let mut words: Vec<(&String, u32)> = dict.iter()
			.filter_map(|(k, v)|
				Some ( (k, words.get(v).map(|e| *e)?) )
			).collect();

		words.sort_by(|(_, a), (_, b)| b.cmp(a));

		let words = words
			.into_iter()
			.map(|(k, _)| k)
			.filter (|k| k.len() >= 3)
			.take(3000);

		let words: Vec<&String>  = stream::iter(words)
			.filter_map(|w| async move {
				let a = word_parts(w).await?;

				info!("POS for {}: {:?}", w, a);

				if a.contains(&PartOfSpeech::Conjunction) ||
					a.contains(&PartOfSpeech::Preposition) {

					None
				} else {
					Some (w)
				}
			})
			.collect()
			.await;

		for word in words {
			f.write_all(format!("{}\n", word).as_bytes()).unwrap();
		}

		f.flush().unwrap();
	};

	if let Err(_) = cexist {
		info!("Generating correlation data...");
		let corr = Correlation::new(dat, fa.len(), &cpath, &dict).unwrap();

		let fw = BufWriter::new(File::create(&cind).unwrap());
		bincode::serialize_into(fw, &corr).unwrap();
	}
}

#[tokio::main(flavor = "current_thread")]
async fn main () {
	log4rs::init_file("log/config.yaml", Default::default()).unwrap();

	info!("Initiated Logger");

	let dict = load_dict(DBDICT).unwrap();

	// this will be discarded as it is already serialized
	gen_word_frequency("frequency", &dict, 0).await;

	let srv = Server::new("").unwrap();
	let conf = Config::build(Environment::active().unwrap())
		.address("127.0.0.1")
    .port(8000)
    .log_level(LoggingLevel::Normal)
    .unwrap();

	let app = rocket::custom(conf);

	error!("Launch failed on {}", srv.mount("/", app).launch());
}


#[cfg(test)]
mod test {
	use regex::Regex;
	use bzip2::bufread::BzDecoder;
	use std::io::{Read, BufRead};
	use super::*;

	const EPSILON: f64 = 1e-12;
	
	#[test]
	/// Test database serialization and deserialization.
	fn database_serialize_deserialize() {
		let db = File::open(format!("{}.bz2", DBDATA)).unwrap();

		// db.seek(SeekFrom::Start(ind[ARTICLEID].0)).unwrap();

		// db.seek(SeekFrom::Start(2369839923)).unwrap();

		let db = BufReader::new(db);
		let db = MultiBzDecoder::new(db);
		// let db = File::open(DBDATA).unwrap();
		let db = BufReader::new(db);
		let db = Database::new(db);

		let mut a = db.into_iter();

		let dict = load_dict(DBDICT).unwrap();

		let mut fa = Frequency::new("results/frequency.dat", &dict).unwrap();

		let mut c = 0;
		while let Some(e) = a.next() {
			fa.insert(e.unwrap().text).unwrap();

			c += 1;
			info!(target: "app::basic", "Parsed article {}", c);

			if c % 1000 == 0 {
				println!("Parsed article {}", c);
			}

			// if c > 100_000 { break; }
		}

		let fon = "results";
		std::fs::create_dir_all(fon).unwrap();
		let fon = &format!("{}/frequency-index.dat", fon);

		let fw = BufWriter::new(File::create(fon).unwrap());

		bincode::serialize_into(fw, &fa).unwrap();

		let fr = File::open(fon).unwrap();

		// Deserialize from the file.
		let mut fad = bincode::deserialize_from(fr).unwrap();

		// Assert that the data of both databases are equal.
		assert_eq!(fa, fad);
		// not writable
		assert_eq!(
			fad.insert(String::from("")),
			Err(database::error::ErrorKind::MissingDict.into())
		);

		fad.set_dict(&dict);
		// now it should be writable
		assert_eq!(fad.insert(String::from("")), Ok( () ));
	}

	#[test]
	/// Load the index file into memory.
	fn index_read () {
		let indexmap: Regex = Regex::new(r"^(\d+):(\d+):(.+)$").unwrap();

		let ind = File::open(format!("{}.bz2", DBINDEX)).unwrap();
		let ind = BufReader::new(ind);
		let ind = BzDecoder::new(ind);
		let ind = BufReader::new(ind);

		let ind: Vec<(u64, u32, String)> = ind.lines()
			.map(|e| e.unwrap())
			.map(|e| {
				let s = indexmap.captures_iter(&e).next().unwrap();

				(
					s[1].parse::<u64>().unwrap(), 
					s[2].parse::<u32>().unwrap(), 
					s[3].to_owned()
				)
			})
			.collect();

		const ARTICLEID: usize = 921235 + 70611;

		println!("Parsing starting at article {} id {}, {:?}, byte {}", ARTICLEID, ind[ARTICLEID].1, ind[ARTICLEID].2, ind[ARTICLEID].0);
	}

	#[test]
	/// Test parse_wiki_text
	fn wikitext () {
		let mut tmp = File::open("tmp.log").unwrap();
		let mut contents = String::new();
		tmp.read_to_string(&mut contents).unwrap();

		println!("{:?}", database::read::CONFIG.parse(&contents));
	}

	#[test]
	/// Correlation test
	fn corr () {
		let dict = load_dict("data/words").unwrap();
		let dat: HashMap<u32,Vec<(u32,u16)>> = HashMap::from([
			(0, vec![(0, 1), (1, 1), (9, 2)]),
			(1, vec![(0, 1), (1, 1), (9, 2)]),
			(2, vec![(9, 1)]),
			(3, vec![(5, 1)])
		]);

		let mut c = Correlation::new(dat, 10, "results/_test/corr.dat", &dict).unwrap();

		println!("{:?}", c.dict());

		let acorn = c.corrall("a").unwrap();

		let corr = |c: &mut Correlation, a:&str, b:&str, exp: f64| {
			let co = c.corr(a,b).unwrap_or(0.);
			println!("{}/{}: {} ({})", a, b, co, exp);

			assert!((co - exp).abs() < EPSILON);
		};

		corr(&mut c, "A","A's", 1.);
		corr(&mut c, "AMD","A", 0.804030252207);
		corr(&mut c, "AMD's","AMD", -0.111111111111);

		let i = c.index("A's").unwrap() as usize;
		corr(&mut c, "A", "A's", acorn[i]);
		let i = c.index("AMD").unwrap() as usize;
		corr(&mut c, "A", "AMD", acorn[i]);
		let i = c.index("Amd's").unwrap() as usize;
		corr(&mut c, "A", "AMD's", acorn[i]);
		let i = c.index("A").unwrap() as usize;
		corr(&mut c, "A", "A", acorn[i]);
	}

	#[test]
	/// deserialize serialize everything
	fn deser () {
		println!("test start");
		let path = Path::new("results").join("frequency");

		let root = path.join("index.dat");
		let cind = path.join("corrindex.dat");

		println!("{}, {}",
			root.to_str().unwrap(),
			cind.to_str().unwrap());

		let fa: Correlation = bincode::deserialize_from(File::open(&cind).unwrap()).unwrap();
		println!("Deser");
		let fw = BufWriter::new(File::create(&cind).unwrap());
		bincode::serialize_into(fw, &fa).unwrap();
		println!("Ser");

		let fa: Frequency = bincode::deserialize_from(File::open(&root).unwrap()).unwrap();
		println!("Deser");
		let fw = BufWriter::new(File::create(&root).unwrap());
		bincode::serialize_into(fw, &fa).unwrap();
		println!("Ser");
	}

	#[tokio::test]
	/// api request
	async fn fetch () {
		let words = vec![
			"cow",
			"fetus",
			"oblong",
			"quickly",
			"and",
			"then",
			"but",
			"the",
			"a",
			"obtuse",
			"bestial",
			"ewohgoijf"
		];

		for word in words {
			println!("{}: {:?}", word, word_parts(word).await);
		}
	}
}
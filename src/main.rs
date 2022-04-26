use std::path::Path;
use server::{
	Launch,
	rocket::{
		self,
		config::{Config, Environment}
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
use std::io::{BufWriter, BufReader, Seek, SeekFrom};
use bzip2::bufread::{MultiBzDecoder};
use std::fs::File;

use log::{info, error};
use const_format::formatcp;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn gen_word_frequency<'a> (namespace: &str, dict: &'a Dict, start: u64) -> Correlation{
	let path = Path::new("results").join(namespace);

	let root = path.join("index.dat");
	let cind = path.join("corrindex.dat");
	let cpath = path.join("corr.dat");

	if let Ok(f) = File::open(&cind) {
		info!("Correlation data already exists, loading.");
		return bincode::deserialize_from(f).unwrap();
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

	info!("Generating correlation data...");
	let corr = Correlation::new(dat, fa.len(), &cpath, &dict).unwrap();

	let fw = BufWriter::new(File::create(&cind).unwrap());
	bincode::serialize_into(fw, &corr).unwrap();

	corr
}

fn main() {
	log4rs::init_file("log/config.yaml", Default::default()).unwrap();

	info!("Initiated Logger");

	let dict = load_dict("data/words").unwrap();

	// this will be discarded as it is already serialized
	let _ = gen_word_frequency("frequency", &dict, 0);

	let srv = Server::new("").unwrap();
	let conf = Config::build(Environment::active().unwrap())
		.address("127.0.0.1")
    .port(8000)
    .unwrap();

	let app = rocket::custom(conf);

	let app = srv.mount("/", app);
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

		let dict = load_dict("data/words").unwrap();

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

		let mut corr = |a:&str, b:&str, exp: f64| {
			let co = c.corr(a,b).unwrap_or(0.);
			println!("{}/{}: {} ({})", a, b, co, exp);

			assert!((co - exp).abs() < EPSILON);
		};

		corr("A","A's", 1.);
		corr("AMD","A", 0.804030252207);
		corr("AMD's","AMD", -0.111111111111);
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
}
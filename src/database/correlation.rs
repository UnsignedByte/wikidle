/// Module managing correlation data
use std::path::{Path, PathBuf};
use core::hash::{Hasher, Hash};
use std::collections::{HashSet, HashMap};
use threadpool::ThreadPool;
use super::read::{Dict};
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess, SeqAccess};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use core::fmt::{Formatter, Debug};
use std::io::{BufWriter, BufReader, Write, Read, Seek, SeekFrom};
use std::fs::File;
use log::{debug, trace};
use super::error::*;
use std::sync::{Arc,RwLock};

/// Structure storing correlation data
pub struct Correlation {
	fname: PathBuf,
	reader: BufReader<File>,
	dict: Dict,
}

impl Correlation {
	/// Generates a new correlation database from raw exported frequency data.
	pub fn new <P: AsRef<Path>> (mut dat: HashMap<u32, Vec<(u32, u16)>>, len: usize, fname: P, dict: &Dict) -> Result<Correlation> {

		debug!(target: "app::dump", "Current dict size {}", dict.len());

		// filter out words that dont appear in wikipedia.
		let nd: Dict = dict.iter()
			.filter(|(_, v)| dat.contains_key(v))
			.map(|(k, _)| k)
			.enumerate()
			.map(|(a, b)| (b.to_owned(), a as u32))
			.collect();

		debug!(target: "app::dump", "Pruned dict to size {}", nd.len());

		let mut w = BufWriter::new(File::create(&fname).map_err(|_| ErrorKind::Io)?);

		let mut ndk: Vec<(&String, &u32)> = nd
			.iter()
			.collect();
		
		ndk.sort_by(|(_, a), (_, b)| (*a).cmp(*b));
		
		let ndk: Vec<u32> = ndk
			.into_iter()
			.map(|(k, _)| k)
			.map(|k| *dict.get(k).unwrap())
			.collect();

		// calculate sums 
		let nds: Vec<f64> = ndk.iter()
			.map(|k| dat.get(k).unwrap())
			.map(|d| 
				d.into_iter()
					.map(|(_, v)| *v as f64)
					.sum::<f64>()
				/ len as f64
			)
			.collect();

		trace!(target: "app::dump", "Most common word appeared {} times on avg.", *nds.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.));

		let sum = ndk.iter()
			.map(|k| dat.get(k).unwrap())
			.enumerate()
			.map(|(i, a)| (a.into_iter()
				.map(|(_, c)| *c as f64 - nds[i])
				.collect::<Vec<f64>>(), a.len())
			);

		let sum2: Vec<f64> = sum
			.clone()
			.enumerate()
			.map(|(i, (e, l))| e.into_iter()
				.map(|n| n * n)
				.sum::<f64>()
				+ (len - l) as f64 * nds[i] * nds[i]
			).collect();

		let sum: Vec<f64> = sum
			.map(|(e, _)| e
				.into_iter().sum()
			).collect();

		debug!(target: "app::dump", "Generated presums");

		struct Ac(u32, f64);

		impl Hash for Ac {
			fn hash<H> (&self, h: &mut H) 
				where H: Hasher, {
					self.0.hash(h);
			}
		}

		impl PartialEq for Ac {
			fn eq(&self, other: &Self) -> bool {
				self.0 == other.0
			}
		}

		impl Eq for Ac {}

		// consumes data and converts it to hashset form
		let uniq: Vec<Arc<HashSet<Ac>>> = ndk.into_iter()
			.map(|e| dat.remove(&e).unwrap())
			.map(|e| e.into_iter()
				.map(|(a,b)| Ac(a,b as f64))
				.collect::<HashSet<Ac>>()
			)
			.map(Arc::new)
			.collect();

		debug!(target: "app::dump", "Generated hashsets");

		let nds = Arc::new(nds);
		let sum = Arc::new(sum);
		let sum2 = Arc::new(sum2);

		// println!("{nds:?}\n{sum:?}\n{sum2:?}");

		let pool: Option<ThreadPool> = match nd.len() {
			0..=1000 => None,
			_ => Some(ThreadPool::new(4))
		};

		for i in 0 .. nd.len() {
			debug!("Parsing word {}.", i);

			type SharedDat = (usize, usize, Arc<HashSet<Ac>>, Arc<Vec<f64>>, Arc<Vec<f64>>, Arc<Vec<f64>>);

			let shared: Arc<SharedDat> = Arc::new( (
				i,
				len,
				Arc::clone(&uniq[i]),
				Arc::clone(&nds),
				Arc::clone(&sum),
				Arc::clone(&sum2),
			) );

			let calc = |j: usize, buf: Arc<RwLock<f64>>, shared: Arc<SharedDat>, b: Arc<HashSet<Ac>>| {
				let b = &*b;
				let (
					i,
					len,
					a,
					nds,
					sum,
					sum2,
				) = (
					shared.0,
					shared.1,
					&*shared.2,
					&*shared.3,
					&*shared.4,
					&*shared.5,
				);

				// calculate numerator ignoring intersection.
				let mut num = sum[i] * -nds[j] + sum[j] * -nds[i];
				let mut nc = 0; // number of shared articles

				let mut iter =  |(a, i): (&HashSet<Ac>, usize), (b, j): (&HashSet<Ac>, usize)| {
					for t in a.iter() {
						if let Some(t2) = b.get(t) {
							let (da, db) = (t.1 - nds[i], t2.1 - nds[j]);

							num += da * nds[j];
							num += db * nds[i];
							num += da * db;
							nc += 1;
						}
					}
				};

				if a.len() < b.len() {
					iter((a, i), (b, j));
				} else {
					iter((b, j), (a, i));
				}

				let nc = (len - a.len() - b.len() + nc) as f64;

				let num = num + nc * nds[i] * nds[j];
				// println!("{}\t{}", nc, num);

				// pearsons r correlation
				let r = num / (sum2[i] * sum2[j]).sqrt();
				trace!(target: "app::dump", "{}:{} {}:{};\t{}", i, a.len(), j, b.len(), r);
				// println!("{}:{} {}:{};\t{}", i, a.len(), j, b.len(), r);

				*buf.write().unwrap() = r;
			};

			let mut buf: Vec<Arc<RwLock<f64>>> = Vec::new();

			for j in 0 .. i {

				let shared = Arc::clone(&shared);
				buf.push(Arc::new(RwLock::new(0.)));
				let slice = Arc::clone(&buf[j]);

				let b = Arc::clone(&uniq[j]); // b freq data

				let cc = move || calc(j, slice, shared, b);
				match pool {
					Some(ref p) => {
						p.execute(cc)
					},
					None => cc (),
				};
			}

			if let Some(ref p) = pool {
				p.join();
			}

			let buf: Vec<u8> = buf.iter()
				.map(|e| *e.read().unwrap())
				.map(|e| e.to_be_bytes())
				.flatten()
				.collect();

			w.write(&buf)
				.map_err(|_| ErrorKind::Io)?;
		}

		Ok(Correlation {
			reader: BufReader::new(File::open(&fname).map_err(|_| ErrorKind::Io)?),
			fname: fname.as_ref().canonicalize().map_err(|_| ErrorKind::Io)?,
			dict: nd
		})
	}

	/// Used to load a correlation database from an existing file
	fn deserialize <P: AsRef<Path>> (fname: P, dict: Dict) -> Result<Correlation> {
		Ok(Correlation {
			reader: BufReader::new(File::open(&fname).map_err(|_| ErrorKind::Io)?),
			fname: fname.as_ref().canonicalize().map_err(|_| ErrorKind::Io)?,
			dict
		})
	}

	/// Index of a word in the dictionary.
	pub fn index (&self, a: &str) -> Option<u32> {
		self.dict.get(&a.to_lowercase()).map(|e| *e)
	}

	/// Find the f64 index of a word pair correlation.
	///
	/// Value should be multiplied by 8 to get the byte index.
	fn find (&self, a: u64, b: u64) -> Option<u64>{
		if a == b {
			return None
		}
		let (a, b) = if a < b { (b, a) } else { (a, b) };

		// a should be > b

		Some(a * (a - 1) / 2 + b)
	}

	/// Returns the pearson's r correlation between two words.
	pub fn corr (&mut self, a: &str, b: &str) -> Option<f64> {
		if a == b {
			return Some(1.)
		}

		let a = self.index(a)? as u64;
		let b = self.index(b)? as u64;

		let ind = self.find(a, b).unwrap();

		self.reader.seek(SeekFrom::Start(ind * 8)).ok()?;

		let mut buf: [u8; 8] = [0; 8];

		self.reader.read_exact(&mut buf).ok()?;

		Some(f64::from_be_bytes(buf))
	}

	pub fn corrall (&mut self, a: &str) -> Option<Vec<f64>> {
		let a = self.index(a)? as u64;

		let ind = self.find(a, 0).unwrap_or(0);

		self.reader.seek(SeekFrom::Start(ind * 8)).ok()?;

		let mut buf: Vec<u8> = vec![0; self.dict.len() * 8];

		self.reader.read_exact(&mut buf[0..a as usize * 8]).ok()?;

		for b in a+1..self.dict.len() as u64 {
			self.reader.seek(SeekFrom::Start(self.find(b, a)? * 8)).ok()?;
			let b = b as usize * 8;
			self.reader.read_exact(&mut buf[b..b+8]).ok()?;
		}

		let mut ret: Vec<f64> = vec![0.; self.dict.len()];

		for i in 0..self.dict.len() {
			let t : [u8; 8] = buf[i*8..i*8+8].try_into().unwrap();
			ret[i] = f64::from_be_bytes(t);
		}

		ret[a as usize] = 1.;

		Some(ret)
	}

	pub fn dict<'a>(&'a self) -> &'a Dict {
		&self.dict
	}
}

impl PartialEq for Correlation {
	fn eq(&self, r: &Correlation) -> bool {
		self.fname == r.fname
	}
}

impl Debug for Correlation {
	fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), std::fmt::Error> {
		f.debug_struct("Correlation")
			.field("fname", &self.fname)
			.finish()
	}
}

impl Serialize for Correlation {
	fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
		where S: Serializer
	{ 
		let mut s = serializer.serialize_struct("Correlation", 2)?;
		s.serialize_field("fname", &self.fname)?;
		s.serialize_field("dict", &self.dict)?;
		s.end()
	}
}

impl<'de> Deserialize<'de> for Correlation {
	fn deserialize<D>(deserializer: D) -> std::result::Result<Correlation, D::Error>
		where D: Deserializer<'de>,
	{
		// implementation following https://serde.rs/deserialize-struct.html

		#[derive(serde::Deserialize)]
		#[serde(field_identifier, rename_all = "lowercase")]
   	enum Field { Fname, Dict }

		struct CorrelationVisitor;

		impl<'de> Visitor<'de> for CorrelationVisitor {
			type Value = Correlation;

			fn expecting(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
				formatter.write_str("A struct representing Correlation analysis data.")
			}

			fn visit_seq<V>(self, mut seq: V) -> std::result::Result<Self::Value, V::Error>
				where V: SeqAccess<'de>,
			{
				let fname: PathBuf = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				let dict: Dict = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				Ok(Correlation::deserialize(&fname, dict)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&format!("Unexpected path: <{}>", fname.to_str().unwrap_or("none"))),
						&"A valid filepath."
					))?)
			}

			fn visit_map<V>(self, mut map: V) -> std::result::Result<Self::Value, V::Error>
        where V: MapAccess<'de>,
      {
      	let mut fname = None;
      	let mut dict = None;

      	while let Some(key) = map.next_key()? {
      		match key {
      			Field::Fname => {
      				if fname.is_some() {
      					return Err(de::Error::duplicate_field("fname"));
      				}

      				fname = Some(map.next_value()?);
      			},
      			Field::Dict => {
      				if dict.is_some() {
      					return Err(de::Error::duplicate_field("dict"));
      				}

      				dict = Some(map.next_value()?);
      			}
      		}
      	}

      	let fname: PathBuf = fname.ok_or_else(|| de::Error::missing_field("fname"))?;
      	let dict: Dict = dict.ok_or_else(|| de::Error::missing_field("dict"))?;

				Ok(Correlation::deserialize(&fname, dict)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&format!("Unexpected path: <{}>", fname.to_str().unwrap_or("none"))),
						&"A valid filepath."
					))?)
      }
		}

    const FIELDS: &'static [&'static str] = &["fname", "index"];
		deserializer.deserialize_struct("Correlation", FIELDS, CorrelationVisitor)
	}
}
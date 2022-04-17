/// Module managing correlation data
use std::collections::{HashSet, HashMap};
use threadpool::ThreadPool;
use crate::{Dict, Frequency};
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess, SeqAccess};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use core::fmt::{Formatter, Debug};
use std::io::{BufWriter, BufReader, Write, Read, Seek, SeekFrom};
use std::fs::File;
use log::{debug, trace};
use super::error::*;
use std::sync::{Arc,RwLock,RwLockWriteGuard,RwLockReadGuard};

/// Structure storing corelation data
pub struct Correlation {
	fname: String,
	reader: BufReader<File>,
	dict: Dict,
}

impl Correlation {
	pub fn new(dat: &HashMap<u32, Vec<(u32, u16)>>, len: usize, fname: &str, dict: &Dict) -> Result<Correlation> {

		debug!(target: "app::dump", "Current dict size {}", dict.len());

		// filter out words that dont appear in wikipedia.
		let nd: Dict = dict.iter()
			.filter(|(_, v)| dat.contains_key(v))
			.map(|(k, _)| k)
			.enumerate()
			.map(|(a, b)| (b.to_owned(), a as u32))
			.collect();

		debug!(target: "app::dump", "Pruned dict to size {}", nd.len());

		let mut w = BufWriter::new(File::create(fname).map_err(|_| ErrorKind::Io)?);

		let mut ndk: Vec<(&String, &u32)> = nd
			.iter()
			.collect();
		
		ndk.sort_by(|(_, a), (_, b)| (*a).cmp(*b));
		
		let ndk: Vec<u32> = ndk
			.iter()
			.map(|(k, _)| k)
			.map(|k| *dict.get(*k).unwrap())
			.collect();

		// calculate sums 
		let nds: Vec<f64> = ndk.iter()
			.map(|k| dat.get(k).unwrap())
			.map(|d| 
				d.iter()
					.map(|(_, v)| *v as f64)
					.sum::<f64>()
				/ len as f64
			)
			.collect();

		trace!(target: "app::dump", "Most common word appeared {} times on avg.", *nds.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.));

		let sum = ndk.iter()
			.map(|k| dat.get(k).unwrap())
			.enumerate()
			.map(|(i, a)| (a.iter()
				.map(|(_, c)| *c as f64 - nds[i])
				.collect::<Vec<f64>>(), a.len())
			);

		let sum2: Vec<f64> = sum
			.clone()
			.enumerate()
			.map(|(i, (e, l))| e.iter()
				.map(|n| n * n)
				.sum::<f64>()
				+ (len - l) as f64 * nds[i] * nds[i]
			).collect();

		let sum: Vec<f64> = sum
			.map(|(e, _)| e
				.iter().sum()
			).collect();

		let ndk = Arc::new(ndk);
		let nds = Arc::new(nds);
		let sum = Arc::new(sum);
		let sum2 = Arc::new(sum2);

		// println!("{nds:?}\n{sum:?}\n{sum2:?}");

		let pool: Option<ThreadPool> = match nd.len() {
			0..=1000 => None,
			_ => Some(ThreadPool::new(4))
		};

		for i in 0 .. nd.len() {
			let a = Arc::new(dat.get(&ndk[i]).unwrap().clone());

			type SharedDat = (usize, usize, Arc<Vec<(u32,u16)>>, Arc<Vec<f64>>, Arc<Vec<f64>>, Arc<Vec<f64>>);

			let shared: Arc<SharedDat> = Arc::new( (
				i,
				len,
				a.clone(),
				nds.clone(),
				sum.clone(),
				sum2.clone(),
			) );

			let calc = |j: usize, buf: Arc<RwLock<f64>>, shared: Arc<SharedDat>, b: Vec<(u32,u16)>| {
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

				let (mut ii, mut jj) = (0,0);

				while ii < a.len() && jj < b.len() {
					let (aid, ac) = a[ii];
					let (bid, bc) = b[jj];

					if aid == bid {
						let (ac, bc) = (ac as f64, bc as f64);
						let (da, db) = (ac - nds[i], bc - nds[j]);

						num += da * nds[j];
						num += db * nds[i];
						num += da * db;
						nc += 1;
					}

					if aid <= bid {
						ii += 1;
					} 

					if aid >= bid {
						jj += 1;
					}
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

				let b: Vec<(u32, u16)> = dat.get(&ndk[j]).unwrap().clone(); // b freq data

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
			fname: fname.to_owned(),
			reader: BufReader::new(File::open(fname).map_err(|_| ErrorKind::Io)?),
			dict: nd
		})
	}

	pub fn deserialize(fname: &str, dict: Dict) -> Result<Correlation> {
		Ok(Correlation {
			fname: fname.to_owned(),
			reader: BufReader::new(File::open(fname).map_err(|_| ErrorKind::Io)?),
			dict
		})
	}

	pub fn corr(&mut self, a: &str, b: &str) -> Option<f64> {
		if a == b {
			return Some(1.)
		}

		let a = *self.dict.get(&a.to_lowercase())? as u64;
		let b = *self.dict.get(&b.to_lowercase())? as u64;

		let (a, b) = if a < b { (b, a) } else { (a, b) };

		// a should be > b

		let ind = a * (a - 1) / 2 + b;

		// println!("{}, {}, {}", a, b, ind);

		self.reader.seek(SeekFrom::Start(ind * 8)).ok()?;

		let mut buf: [u8; 8] = [0; 8];

		self.reader.read_exact(&mut buf).ok()?;

		Some(f64::from_be_bytes(buf))
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
			.field("dict", &self.dict)
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
				let fname: String = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				let dict: Dict = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				Ok(Correlation::deserialize(&fname, dict)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&fname),
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

      	let fname: String = fname.ok_or_else(|| de::Error::missing_field("fname"))?;
      	let dict: Dict = dict.ok_or_else(|| de::Error::missing_field("dict"))?;

				Ok(Correlation::deserialize(&fname, dict)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&fname),
						&"A valid filepath."
					))?)
      }
		}

    const FIELDS: &'static [&'static str] = &["fname", "index"];
		deserializer.deserialize_struct("Correlation", FIELDS, CorrelationVisitor)
	}
}
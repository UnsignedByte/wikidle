/// Module managing correlation data
use std::collections::{HashSet, HashMap};
use threadpool::ThreadPool;
use crate::{Dict, Frequency};
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess, SeqAccess};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use core::fmt::{Formatter, Debug};
use std::io::{BufWriter, BufReader, Write, BufRead, Seek, SeekFrom};
use std::fs::File;
use log::{debug, trace};
use super::error::*;

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

		let ndk: Vec<u32> = nd
			.keys()
			.map(|k| *dict.get(k).unwrap())
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

		// let pool = ThreadPool::new(4);

		let sum = ndk.iter()
			.map(|k| dat.get(k).unwrap())
			.enumerate()
			.map(|(i, a)| a.iter()
				.map(|(_, c)| *c as f64 - nds[i])
				.collect::<Vec<f64>>()
			);

		let sum2: Vec<f64> = sum
			.clone()
			.map(|e| e.iter()
				.map(|n| n * n)
				.sum()
			).collect();

		let sum: Vec<f64> = sum.map(|e| e.iter().sum()).collect();

		for i in 0 .. nd.len() {
			let a = dat.get(&ndk[i]).unwrap();

			for j in 0 .. i {
				let b = dat.get(&ndk[j]).unwrap(); // b freq data

				// calculate numerator ignoring intersection.
				let mut num = sum[i] * nds[j] + sum2[j] * nds[i];

				let (mut ii, mut jj) = (0,0);

				while ii < a.len() && jj < b.len() {
					let (aid, ac) = a[ii];
					let (bid, bc) = b[jj];

					if aid == bid {
						let (ac, bc) = (ac as f64, bc as f64);
						let (da, db) = (ac - nds[i], bc - nds[j]);

						num -= da * nds[j];
						num -= db * nds[i];
						num += da * db;
					}

					if aid <= bid {
						ii += 1;
					} 

					if aid >= bid {
						jj += 1;
					}
				}

				// pearsons r correlation
				let r = num / (sum2[i] * sum2[j]).sqrt();
				trace!(target: "app::dump", "Word {} and {} corr {}", i, j, r);

				w.write(&r.to_be_bytes())
					.map_err(|_| ErrorKind::Io)?;
			}
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
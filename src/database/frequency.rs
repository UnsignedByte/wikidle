/// Analyzes the wikipedia database
use crate::Dict;
use core::fmt::{Formatter, Debug};
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess, SeqAccess};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::{ HashMap };
use regex::Regex;
use lazy_static::lazy_static;
use std::io::{BufWriter, BufReader, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use log::{debug, trace};
use super::error::*;

lazy_static! {
	/// Static regex for parsing words.
	static ref WORD: Regex = Regex::new(r"\b[^\s]+\b").unwrap();
}

/// Struct representing the frequency analysis of words in the database.
pub struct Frequency<'a> {
	fname: String,
	writer: BufWriter<File>,
	reader: BufReader<File>,
	index: Vec<u64>,
	dict: Option<&'a Dict>,
}

impl<'a> Frequency<'a> {
	/// Create a new empty frequency data table with a dictionary.
	pub fn new ( fname: &str, dict: &'a Dict ) -> Result<Frequency<'a>> {
		Ok(Frequency {
			writer: BufWriter::new(File::create(fname).map_err(|_| ErrorKind::Io)?),
			reader: BufReader::new(File::open(fname).map_err(|_| ErrorKind::Io)?),
			fname: fname.to_owned(),
			index: Vec::new(),
			dict: Some(dict),
		})
	}

	/// Load a read-only frequency data table from data.
	fn deserialize ( fname: &str, index: Vec<u64> ) -> Result<Frequency<'static>> {
		Ok(Frequency {
			writer: BufWriter::new(OpenOptions::new().append(true).open(fname).map_err(|_| ErrorKind::Io)?),
			reader: BufReader::new(File::open(fname).map_err(|_| ErrorKind::Io)?),
			fname: fname.to_owned(),
			index,
			dict: None,
		})
	}

	/// Make a frequency table writable by loading a dict
	pub fn set_dict ( &mut self, dict: &'a Dict ) {
		self.dict = Some(dict);
	}

	/// Parses a string to find all occurrences of valid words.
	///
	/// Arguments
	/// * `article`: A string representing the article to parse for words.
	///
	/// Returns
	/// * `Err(ErrorKind::MissingDict)` if the dictionary is undefined,
	/// 	usually occurring if the data has been loaded from file.
	/// * `Ok( () )` if parsed properly
	pub fn insert ( &mut self, article: String ) -> Result<()> {
		let dict = self.dict.ok_or_else(|| ErrorKind::MissingDict)?;
		let mut data: HashMap<u32,u16> = HashMap::new();

		self.index.push(self.writer.stream_position()
			.map_err(|_| ErrorKind::Io)?);

		debug!("Loading article {} with {} chars.", self.index.len(), article.len());
		trace!(target: "app::dump", "raw article:\n{}", article);

		for word in WORD.captures_iter(&article) {
			let word = &word[0];
			let word = word.to_lowercase();

			if let Some(i) = dict.get(&word) {
				*data.entry(*i)
					.or_insert(0) += 1;
			}
		};

		let u = bincode::serialized_size(&data)
			.map_err(|_| ErrorKind::Serialization)?;

		debug!("Finished article {}, writing {} bytes.", self.index.len(), u);

		debug!("Database size {}.", self.writer.stream_position().map_err(|_| ErrorKind::Io)?);

		bincode::serialize_into(&mut self.writer, &data).map_err(|_| ErrorKind::Serialization)?;

		Ok(())
	}

	// Writes the serializer to a file organized by word
	pub fn load( &mut self ) -> Result<HashMap<u32, Vec<(u32, u16)>>> {
		let mut map: HashMap<u32, Vec<(u32, u16)>> = HashMap::new();

		for id in 0..self.index.len() {
			let start = self.index[id];

			trace!(target: "app::dump", "loading article {} at byte {}", id, start);

			self.reader.seek(SeekFrom::Start(start))
				.map_err(|_| ErrorKind::Io)?;

			let dmap: HashMap<u32,u16> = bincode::deserialize_from(&mut self.reader)
				.map_err(|_| ErrorKind::Serialization)?;

			trace!(target: "app::dump", "Deserialized");

			for (word, count) in dmap.iter() {
				map.entry(*word)
					.or_insert(Vec::new())
					.push( (id as u32, *count) );
			}
		}

		Ok(map)
	}

	/// Get the size of the frequency database (number of inserted articles)
	pub fn len( &self ) -> usize {
		self.index.len()
	}
}

impl PartialEq for Frequency<'_> {
	fn eq(&self, r: &Frequency) -> bool {
		self.fname == r.fname && self.index.len() == r.index.len() && self.index == r.index
	}
}

impl Debug for Frequency<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
		f.debug_struct("Frequency")
			.field("fname", &self.fname)
			.field("index", &self.index)
			.finish()
	}
}

impl Serialize for Frequency<'_> {
	fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
		where S: Serializer
	{ 
		let mut s = serializer.serialize_struct("Frequency", 2)?;
		s.serialize_field("fname", &self.fname)?;
		s.serialize_field("index", &self.index)?;
		s.end()
	}
}

impl<'de> Deserialize<'de> for Frequency<'_> {
	fn deserialize<D>(deserializer: D) -> std::result::Result<Frequency<'static>, D::Error>
		where D: Deserializer<'de>,
	{
		// implementation following https://serde.rs/deserialize-struct.html

		#[derive(serde::Deserialize)]
		#[serde(field_identifier, rename_all = "lowercase")]
   	enum Field { Fname, Index }

		struct FrequencyVisitor;

		impl<'de> Visitor<'de> for FrequencyVisitor {
			type Value = Frequency<'static>;

			fn expecting(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
				formatter.write_str("A struct representing frequency analysis data.")
			}

			fn visit_seq<V>(self, mut seq: V) -> std::result::Result<Self::Value, V::Error>
				where V: SeqAccess<'de>,
			{
				let fname: String = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				let index = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				Ok(Frequency::deserialize(&fname, index)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&fname),
						&"A valid filepath."
					))?)
			}

			fn visit_map<V>(self, mut map: V) -> std::result::Result<Self::Value, V::Error>
        where V: MapAccess<'de>,
      {
      	let mut fname = None;
      	let mut index = None;

      	while let Some(key) = map.next_key()? {
      		match key {
      			Field::Fname => {
      				if fname.is_some() {
      					return Err(de::Error::duplicate_field("fname"));
      				}

      				fname = Some(map.next_value()?);
      			},
      			Field::Index => {
      				if index.is_some() {
      					return Err(de::Error::duplicate_field("index"));
      				}

      				index = Some(map.next_value()?);
      			},
      		}
      	}

      	let fname: String = fname.ok_or_else(|| de::Error::missing_field("fname"))?;
      	let index = index.ok_or_else(|| de::Error::missing_field("index"))?;
				Ok(Frequency::deserialize(&fname, index)
					.map_err(|_| de::Error::invalid_value(
						de::Unexpected::Str(&fname),
						&"A valid filepath."
					))?)
      }
		}

    const FIELDS: &'static [&'static str] = &["fname", "index"];
		deserializer.deserialize_struct("Frequency", FIELDS, FrequencyVisitor)
	}
}
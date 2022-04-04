/// Analyzes the wikipedia database
use core::fmt::{self, Formatter, Debug, Display};
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess, SeqAccess};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::{ HashMap, HashSet };
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
	static ref WORD: Regex = Regex::new(r"\b[^\s]+\b").unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadOnlyError;

impl Display for ReadOnlyError {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
      write!(f, "Attempted to write to read-only frequency database.")
  }
}

/// Struct representing the frequency analysis of words in the database.
pub struct Frequency<'a> {
	data: HashMap<String, HashMap<usize, u16>>,
	dict: Option<&'a HashSet<String>>,
	counter: usize,
}

impl<'a> Frequency<'a> {
	pub fn new ( dict: &'a HashSet<String> ) -> Frequency {
		Frequency {
			data: HashMap::new(),
			dict: Some(dict),
			counter: 0
		}
	}

	/// Load a read-only frequency data table from a file.
	pub fn load ( data: HashMap<String, HashMap<usize, u16>>, counter: usize ) -> Frequency<'a> {
		Frequency { data, counter, dict: None }
	}

	/// Parses a string to find all occurrences of valid words.
	pub fn insert ( &mut self, article: String ) -> Result<(), ReadOnlyError> {
		let dict = self.dict.ok_or_else(|| ReadOnlyError)?;

		self.counter = self.counter + 1;

		for word in WORD.captures_iter(&article) {
			let word = &word[0];
			let word = word.to_lowercase();

			if dict.contains(&word) {
				*self.data.entry(word)
					.or_insert(HashMap::new())
					.entry(self.counter)
					.or_insert(0) += 1;
			}
		};

		Ok(())
	}
}

impl PartialEq for Frequency<'_> {
	fn eq(&self, r: &Frequency) -> bool {
		self.data == r.data && self.counter == r.counter
	}
}

impl Debug for Frequency<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
		f.debug_struct("Frequency")
         .field("data", &self.data)
         .field("counter", &self.counter)
         .finish()
	}
}

impl Serialize for Frequency<'_> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where S: Serializer
	{ 
		let mut s = serializer.serialize_struct("Frequency", 2)?;
		s.serialize_field("data", &self.data)?;
		s.serialize_field("counter", &self.counter)?;
		s.end()
	}
}

impl<'de> Deserialize<'de> for Frequency<'_> {
	fn deserialize<D>(deserializer: D) -> Result<Frequency<'static>, D::Error>
		where D: Deserializer<'de>,
	{
		#[derive(serde::Deserialize)]
		#[serde(field_identifier, rename_all = "lowercase")]
   	enum Field { Data, Counter }

		struct FrequencyVisitor;

		impl<'de> Visitor<'de> for FrequencyVisitor {
			type Value = Frequency<'static>;

			fn expecting(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
				formatter.write_str("A struct representing frequency analysis data.")
			}

			fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
				where V: SeqAccess<'de>,
			{
				let data = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

				let counter = seq.next_element()?
					.ok_or_else(|| de::Error::invalid_length(0, &self))?;

					Ok(Frequency::load(data, counter))
			}

			fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where V: MapAccess<'de>,
      {
      	let mut data = None;
      	let mut counter = None;

      	while let Some(key) = map.next_key()? {
      		match key {
      			Field::Data => {
      				if data.is_some() {
      					return Err(de::Error::duplicate_field("data"));
      				}

      				data = Some(map.next_value()?);
      			},
      			Field::Counter => {
      				if counter.is_some() {
      					return Err(de::Error::duplicate_field("counter"));
      				}

      				counter = Some(map.next_value()?);
      			}
      		}
      	}

      	let data = data.ok_or_else(|| de::Error::missing_field("data'"))?;
      	let counter = counter.ok_or_else(|| de::Error::missing_field("counter'"))?;
      	Ok(Frequency::load(data, counter))
      }
		}

    const FIELDS: &'static [&'static str] = &["data", "counter"];
		deserializer.deserialize_struct("Frequency", FIELDS, FrequencyVisitor)
	}
}
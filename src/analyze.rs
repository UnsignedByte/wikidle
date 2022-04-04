/// Analyzes the wikipedia database
use core::fmt::{Debug, Formatter};
use std::collections::{ HashMap, HashSet };
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
	static ref WORD: Regex = Regex::new(r"\b[^\s]+\b").unwrap();
}

/// Struct representing the frequency analysis of words in the database.
pub struct Frequency<'a> {
	data: HashMap<String, HashMap<usize, u16>>,
	dict: &'a HashSet<String>,
	counter: usize,
}

impl<'a> Frequency<'a> {
	pub fn new ( dict: &'a HashSet<String> ) -> Frequency {
		Frequency {
			data: HashMap::new(),
			dict,
			counter: 0
		}
	}

	/// Parses a string to find all occurrences of valid words.
	pub fn insert ( &mut self, article: String ) {
		self.counter = self.counter + 1;

		for word in WORD.captures_iter(&article) {
			let word = &word[0];
			let word = word.to_lowercase();

			if self.dict.contains(&word) {
				*self.data.entry(word)
					.or_insert(HashMap::new())
					.entry(self.counter)
					.or_insert(0) += 1;
			}
		}
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
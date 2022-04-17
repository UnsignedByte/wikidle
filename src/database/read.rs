use std::fs::File;
use xml::reader::{EventReader, XmlEvent};
use std::collections::{ HashMap };

use std::io::{BufReader, BufRead};
use lazy_static::lazy_static;
use log::{debug, warn, trace};

use super::error::*;

use regex::Regex;

/// Contains config parameters for wikitext
///
/// Generated using [fetch_mediawiki_configuration](https://github.com/brkalmar/fetch_mediawiki_configuration)
const CONFIGPARAMS: parse_wiki_text::ConfigurationSource = parse_wiki_text::ConfigurationSource {
	category_namespaces : & ["category"], 
	extension_tags : & ["categorytree" , "ce" , "charinsert" , "chem" , "gallery" , "graph" , "hiero" , "imagemap" , "indicator" , "inputbox" , "langconvert" , "mapframe" , "maplink" , "math" , "nowiki" , "poem" , "pre" , "ref" , "references" , "score" , "section" , "source" , "syntaxhighlight" , "templatedata" , "templatestyles" , "timeline"],
	file_namespaces : & ["file" , "image"], 
	link_trail : "abcdefghijklmnopqrstuvwxyz", 
	magic_words : & ["disambig" , "expected_unconnected_page" , "expectunusedcategory" , "forcetoc" , "hiddencat" , "index" , "newsectionlink" , "nocc" , "nocontentconvert" , "noeditsection" , "nogallery" , "noglobal" , "noindex" , "nonewsectionlink" , "notc" , "notitleconvert" , "notoc" , "staticredirect" , "toc"], 
	protocols : & ["//" , "bitcoin:" , "ftp://" , "ftps://" , "geo:" , "git://" , "gopher://" , "http://" , "https://" , "irc://" , "ircs://" , "magnet:" , "mailto:" , "mms://" , "news:" , "nntp://" , "redis://" , "sftp://" , "sip:" , "sips:" , "sms:" , "ssh://" , "svn://" , "tel:" , "telnet://" , "urn:" , "worldwind://" , "xmpp:"], 
	redirect_magic_words : & ["redirect"], 
};

lazy_static! {
	/// Static parser for wiki text.
	pub static ref CONFIG: parse_wiki_text::Configuration = parse_wiki_text::Configuration::new(&CONFIGPARAMS);

	static ref DOUBLE_OPEN_CURLY: Regex = Regex::new(r"\{\{").unwrap();
	static ref DOUBLE_CLOSE_CURLY: Regex = Regex::new(r"\}\}").unwrap();
	static ref OPEN_BAR_CURLY: Regex = Regex::new(r"\{\|").unwrap();
	static ref CLOSE_BAR_CURLY: Regex = Regex::new(r"\|\}[^}]").unwrap();
}

pub type Dict = HashMap<String, u32>;

pub fn load_dict(fname: &str) -> Result<Dict> {
	let mut dict: Dict = HashMap::new();

	let df = File::open(fname).map_err(|_| ErrorKind::Io)?;
	let df = BufReader::new(df);

	for (i, l) in df.lines().enumerate() {
		dict.entry(l.map_err(|_| ErrorKind::Io)?.to_lowercase()).or_insert(i as u32);
	}

	Ok(dict)
}

// Type representing a page.
#[derive(Debug,PartialEq,Clone)]
pub struct Page {
	pub id: usize,
	pub namespace: i32,
	pub title: String,
	pub text: String
}

/// An iterator over the database
pub struct Articles<T: BufRead> {
	reader: EventReader<T>,
}

impl<T: BufRead> Articles<T> {
	/// Creates a new iterator over the articles.
	fn new(f: T) -> Articles<T> {
		Articles {
			reader: EventReader::new(f)
		}
	}
}

/// Converts the next article in the article iterator to a string
fn wikitext_as_plaintext (p: &str) -> String {
	fn node_as_plaintext(n: &parse_wiki_text::Node, p: &str) -> String {
		use parse_wiki_text::Node::*;

		trace!(target: "app::dump", "Parsing node {:?}", n);

		trait ListItem {
			fn get_nodes(&self) -> &Vec<parse_wiki_text::Node>;
		}

		impl ListItem for parse_wiki_text::DefinitionListItem<'_> {
			fn get_nodes(&self) -> &Vec<parse_wiki_text::Node> {
				&self.nodes
			}
		}

		impl ListItem for parse_wiki_text::ListItem<'_> {
			fn get_nodes(&self) -> &Vec<parse_wiki_text::Node> {
				&self.nodes
			}
		}

		impl ListItem for parse_wiki_text::TableCell<'_> {
			fn get_nodes(&self) -> &Vec<parse_wiki_text::Node> {
				&self.content
			}
		}

		impl ListItem for parse_wiki_text::TableCaption<'_> {
			fn get_nodes(&self) -> &Vec<parse_wiki_text::Node> {
				&self.content
			}
		}

		fn parse_nodelist (n: &Vec<parse_wiki_text::Node>, p: &str) -> String {
			let mut s = String::from("");

			for node in n {
				s = format!("{}{}", s, node_as_plaintext(&node, p))
			}

			s
		}

		fn parse_listitems<T: ListItem> (i: &Vec<T>, p: &str) -> String {
			if i.len() == 0 { return String::from("") }

			let mut s = format!("[{}", parse_nodelist(i[0].get_nodes(), p));

			if i.len() > 1 {
				for item in &i[1..] {
					s = format!("{}, {}", s, parse_nodelist(item.get_nodes(), p))
				}
			}

			format!("{}]", s)
		}
		
		let mut s: String = String::from("");

		match n {
			Text { value: v, .. } => s += v,

			Italic { start, end } |
			BoldItalic { start, end } |
			Bold { start, end } => s += &p[*start..*end],

			CharacterEntity { character: c, .. } => s = format!("{}{}", s, c),

			Table { captions: c, rows: r, .. } => {
				s += "\n";

				for row in r {
					s = format!("{}\n{}", s, parse_listitems(&row.cells, p)); 
				}

				s = format!("{}\n{}\n", s, parse_listitems(c, p));
			}

			UnorderedList { items: i, .. } |
			OrderedList { items: i, .. } => s += parse_listitems(i, p).as_str(),
			DefinitionList { items: i, .. } => s += parse_listitems(i, p).as_str(),

			Preformatted { nodes: n, .. } |
			Image { text: n, .. } |
			Heading { nodes: n, .. } |
			Link { text: n, .. } |
			ExternalLink { nodes: n, .. } => s += parse_nodelist(n, p).as_str(),

			Category { .. } |
			Template { .. } |
			Tag { .. } |
			Redirect { .. } |
			Parameter { .. } |
			ParagraphBreak { .. } |
			MagicWord { .. } |
			HorizontalDivider { .. } |
			StartTag { .. } |
			EndTag { .. } |
			Comment { .. } => (),
		};

		trace!(target: "app::dump", "Parsed wikitext {:?}", &s);

		s
	}
	
	trace!(target: "app::dump", "Parsing Wikitext {:?}", p);

	let diff = DOUBLE_OPEN_CURLY.captures_iter(&p).count() as i64
		- DOUBLE_CLOSE_CURLY.captures_iter(&p).count() as i64;
	let diff2 = (OPEN_BAR_CURLY.captures_iter(&p).count() as i64
		- CLOSE_BAR_CURLY.captures_iter(&p).count() as i64).abs();

	// If more than 6 unclosed double open brace "{{"
	// or more than 6 unclosed "{|" or "|}" are found, don't parse the wikitext.
	// Prevents parse_wiki_text from rewind()ing and hanging.
	if diff > 6 || diff2 > 6 {
		warn!("Skipping article due to {} mismatched \"{{{{\" and {} \"{{| |}}\".", diff, diff2);

		return p.to_owned();
	}

	let o = CONFIG.parse(p);

	let mut s: String = String::from("");

	trace!(target: "app::dump", "Parsed Wikitext");

	for node in &o.nodes {
		s = format!("{}\n{}", s, node_as_plaintext(node, p));
		// dbg!(&s);
	};

	trace!(target: "app::dump", "Converted to str");

	s
}

impl<T: BufRead> Iterator for Articles<T> {
	type Item = Result<Page>;

	/// The next article in the database.
	///
	/// Returns:
	/// * `Some(Ok(s))` where `s` is the plaintext string containing the article
	/// * `Some(Err(s))` when there was an error reading the XML.
	/// * `None` if the end of the document has been reached
	fn next(&mut self) -> Option<Self::Item> {
		debug!("Reading article.");

		fn article_to_page<T: BufRead>(reader: &mut EventReader<T>) -> Result<Page> {

			enum TagType {
				Close,
				Open
			}

			fn capture_tag<T: BufRead>(reader: &mut EventReader<T>, name: &str) -> Result<String> {
				fn discard_whitespace<T: BufRead>(reader: &mut EventReader<T>) -> Result<XmlEvent> {
					match reader.next().map_err(|_| ErrorKind::XML)? {
						XmlEvent::Whitespace(_) => discard_whitespace(reader),
						x => Ok(x)
					}
				}

				fn match_tag(tag: XmlEvent, name: &str, tt: TagType) -> bool {
					match tt {
						TagType::Open => {
							matches!(tag,
								XmlEvent::StartElement { name: n, .. }
									if n.local_name.as_str() == name
							)
						},
						TagType::Close => {
							matches!(tag,
								XmlEvent::EndElement { name: n }
									if n.local_name.as_str() == name
							)
						}
					}
				}

				// Open title tag
				if !match_tag(discard_whitespace(reader)?,
					name, TagType::Open) { return Err(ErrorKind::XML.into()) }

				let s = if let XmlEvent::Characters(s) = reader.next().map_err(|_| ErrorKind::XML)? {
						s
				} else {
					return Err(ErrorKind::XML.into());
				};

				// Close title tag
				if !match_tag(discard_whitespace(reader)?,
					name, TagType::Close) { return Err(ErrorKind::XML.into()) }

				Ok(s)
			}

			let title = capture_tag(reader, "title")?;
			let ns = capture_tag(reader, "ns")?
				.parse::<i32>().map_err(|_| ErrorKind::XML)?;
			let id = capture_tag(reader, "id")?
				.parse::<usize>().map_err(|_| ErrorKind::XML)?;

			let mut text: Option<String> = None;

			let mut consumer: Option<&mut String> = None;

			// debug!("Searching for page end");

			while let Ok(e) = reader.next() {
				match e {
					XmlEvent::StartElement { name: n, .. } => {
						let n = n.local_name;
						match n.as_str() {
							"text" => consumer = Some(text.get_or_insert(String::new())),
							"page" => return Err(ErrorKind::XML.into()),
							_ => (),
						}
					},
					XmlEvent::EndElement { name: n } => {
						let n = n.local_name;
						match n.as_str() {
							"text" => consumer = None,
							"page" => {
								trace!(target: "app::dump", "Page end found.");
								return Ok(Page {
									text: text
										.ok_or_else(|| ErrorKind::XML)?,
									title,
									namespace: ns,
									id,
								})
							}
							_ => ()
						}
					},
					XmlEvent::Whitespace(s) |
					XmlEvent::Characters(s) |
					XmlEvent::CData(s) => {
						trace!(target: "app::dump", "Concatenating {}", s);
						consumer = if let Some(c) = consumer {
							*c = format!("{}{}", *c, s);
							Some(c)
						} else {
							consumer
						}
					},
					_ => (),
				}
			}

			Err(ErrorKind::XML.into())
		}


		match self.reader.next() {
			Ok(x) => match x {
				XmlEvent::StartElement {
					name: n,
					..
				} if n.local_name.as_str() == "page" => {
					let x = match article_to_page(&mut self.reader) {
						Ok (x) => x,
						Err(x) => return Some(Err(x))
					};

					match x {
						Page { namespace: 0, text: t, .. } => Some(Ok(Page {
							text: wikitext_as_plaintext(&t),
							..x
						})),
						_ => self.next()
					}
				},
				XmlEvent::EndDocument => None,
				_ => self.next()
			},
			Err(_) => Some(Err(ErrorKind::XML.into()))
		}

	}
}

/// Represents the full wikipedia database.
pub struct Database<T: BufRead> {
	articles: Articles<T>,
}

impl<T: BufRead> Database<T> {
	/// Creates a database reading from the specified file path.
	///
	/// # Arguments
	/// * `f` - file name to read from.
	///
	/// # Returns
	/// * `Ok(d)` with a database `d`.
	/// * `Err(e)` with an IO error `e` if the file could not be opened.
	///
	/// # Examples
	///
	/// ```
	/// let db = database::Database::new("wiki.xml").unwrap();
	/// 
	/// assert(db.fname == "wiki.xml");
	/// ```
	pub fn new(f: T) -> Database<T> {
		Database {
			articles: Articles::new(f),
		}
	}
}

impl<T: BufRead> IntoIterator for Database<T> {
	type Item = Result<Page>;
	type IntoIter = Articles<T>;
	fn into_iter(self) -> Self::IntoIter {
		self.articles
	}
}

impl<T: BufRead> std::fmt::Debug for Database<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Database")
		 .finish()
	}
}
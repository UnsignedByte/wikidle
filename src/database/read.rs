use xml::reader::{EventReader, XmlEvent};

use std::io::{BufRead};
use lazy_static::lazy_static;
use log::{debug, trace};

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
	static ref CONFIG: parse_wiki_text::Configuration = parse_wiki_text::Configuration::new(&CONFIGPARAMS);
}

/// Type representing an item of Articles
type Article = xml::reader::Result<String>;

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

	/// Converts the next article in the article iterator to a string
	fn article_as_str (&mut self) -> String {
		let wikitext_as_plaintext = |p: &String| -> String {
			fn node_as_plaintext(n: &parse_wiki_text::Node, p: &str) -> String {
				use parse_wiki_text::Node::*;

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

				// dbg!(&s);

				s
			}
			
			// debug!("Parsing Wikitext");
			let o = CONFIG.parse(&p);

			let mut s: String = String::from("");

			// debug!("Parsed Wikitext");

			for node in &o.nodes {
				s = format!("{}\n{}", s, node_as_plaintext(node, p));
				// dbg!(&s);
			};

			// debug!("Converted to str");

			s
		};


		let mut p: String = String::from("");

		// debug!("Searching for page end");

		while let Ok(e) = self.reader.next() {
			match e {
				XmlEvent::EndElement { name: n } if n.local_name.as_str() == "page" => {
					return wikitext_as_plaintext(&p)
				},
				XmlEvent::Characters(s) |
				XmlEvent::CData(s) => p = format!("{}{}", p, s),
				_ => (),
			}
		}

		panic!("Invalid xml.");
	}
}

impl<T: BufRead> Iterator for Articles<T> {
	type Item = Article;

	/// The next article in the database.
	///
	/// Returns:
	/// * `Some(Ok(s))` where `s` is the plaintext string containing the article
	/// * `Some(Err(s))` when there was an error reading the XML.
	/// * `None` if the end of the document has been reached
	fn next(&mut self) -> Option<Self::Item> {
		debug!("Reading article.");

		match self.reader.next() {
			Ok(x) => match x {
				XmlEvent::StartElement {
					name: n,
					..
				} if n.local_name.as_str() == "page" => Some(Ok(self.article_as_str())),
				XmlEvent::EndDocument => None,
				_ => self.next()
			},
			Err(x) => Some(Err(x))
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
	type Item = Article;
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
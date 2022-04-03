/**
 * Parses wikitext into plaintext articles as a stream.
 */
use parse_wiki_text;
use xml::reader::{EventReader, XmlEvent};
use std::fs::File;

// Generated using https://github.com/brkalmar/fetch_mediawiki_configuration
const CONFIGPARAMS: parse_wiki_text::ConfigurationSource = parse_wiki_text::ConfigurationSource {
	category_namespaces : & ["category"], 
	extension_tags : & ["categorytree" , "ce" , "charinsert" , "chem" , "gallery" , "graph" , "hiero" , "imagemap" , "indicator" , "inputbox" , "langconvert" , "mapframe" , "maplink" , "math" , "nowiki" , "poem" , "pre" , "ref" , "references" , "score" , "section" , "source" , "syntaxhighlight" , "templatedata" , "templatestyles" , "timeline"],
	file_namespaces : & ["file" , "image"], 
	link_trail : "abcdefghijklmnopqrstuvwxyz", 
	magic_words : & ["disambig" , "expected_unconnected_page" , "expectunusedcategory" , "forcetoc" , "hiddencat" , "index" , "newsectionlink" , "nocc" , "nocontentconvert" , "noeditsection" , "nogallery" , "noglobal" , "noindex" , "nonewsectionlink" , "notc" , "notitleconvert" , "notoc" , "staticredirect" , "toc"], 
	protocols : & ["//" , "bitcoin:" , "ftp://" , "ftps://" , "geo:" , "git://" , "gopher://" , "http://" , "https://" , "irc://" , "ircs://" , "magnet:" , "mailto:" , "mms://" , "news:" , "nntp://" , "redis://" , "sftp://" , "sip:" , "sips:" , "sms:" , "ssh://" , "svn://" , "tel:" , "telnet://" , "urn:" , "worldwind://" , "xmpp:"], 
	redirect_magic_words : & ["redirect"], 
};

type Article = xml::reader::Result<String>;

pub struct Articles {
	reader: EventReader<File>,
	config: parse_wiki_text::Configuration,
}

impl Articles {
	fn new(i: File) -> Articles {
		Articles {
			config: parse_wiki_text::Configuration::new(&CONFIGPARAMS),
			reader: EventReader::new(i)
		}
	}

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

				// dbg!(&n);

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
			let o = self.config.parse(&p);

			let mut s: String = String::from("");

			// dbg!(&o);

			for node in &o.nodes {
				s = format!("{}\n{}", s, node_as_plaintext(node, p));
				// dbg!(&s);
			};

			s
		};


		let mut p: String = String::from("");

		while let Ok(e) = self.reader.next() {

			match e {
				XmlEvent::EndElement { name: n } if n.local_name.as_str() == "page" => {
					// dbg!(&p);
					return wikitext_as_plaintext(&p)
				},
				XmlEvent::Whitespace(s) |
				XmlEvent::Characters(s) |
				XmlEvent::CData(s) => {
					// dbg!(&s);
					p += s.as_str()
				},
				_ => (),
			}
		}

		panic!("Invalid xml.");
	}
}

impl Iterator for Articles {
	type Item = Article;
	fn next(&mut self) -> Option<Self::Item> {

		match self.reader.next() {
			Ok(x) => match x {
				XmlEvent::StartElement {
					name: n,
					..
				} if n.local_name.as_str() == "page" => Some(Ok(self.article_as_str())),
				_ => self.next()
			},
			Err(x) => Some(Err(x))
		}
	}
}

pub struct Database<'a> {
	fname: &'a str,
	articles: Articles,
}

impl Database<'_> {
	pub fn new(f: &str) -> Result<Database, std::io::Error> {
		Ok(Database {
			fname: f,
			articles: Articles::new(File::open(f)?),
		})
	}
}

impl IntoIterator for Database<'_> {
	type Item = Article;
	type IntoIter = Articles;
	fn into_iter(self) -> Self::IntoIter {
		self.articles
	}
}

impl std::fmt::Debug for Database<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
         .field("fname", &self.fname)
         .finish()
    }
}
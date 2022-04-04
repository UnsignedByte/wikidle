mod test {
	use crate::*;

	#[test]
	fn database_fname() {
    let s = "enwiki-20220101-pages-articles-multistream";

    let s = format!("data/{}/{0}.xml", s);
    let db = database::Database::new(&s).unwrap();
	 
		assert!(db.fname == s);
	}
}
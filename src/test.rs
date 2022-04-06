mod test {
    use crate::*;
	use std::io::Read;
    use log::{debug};

	#[test]
    /// Test database serialization and deserialization.
	fn database_serialize_deserialize() {
        let db = database::Database::new(DBDATA).unwrap();
    	 
        let mut a = db.into_iter();

        let mut dict: HashSet<String> = HashSet::new();

        let df = "data/words";
        let df = File::open(df).expect(&format!("Missing {:?} file.", df));
        let df = BufReader::new(df);

        for l in df.lines() {
            dict.insert(l.unwrap().to_lowercase());
        }


        let mut fa = analyze::Frequency::new(&dict);

        for _ in 1..5 {
        	fa.insert(a.next().unwrap().unwrap()).unwrap();
        }

        let fon = "results/_test";
        std::fs::create_dir_all(fon).unwrap();
        let fon = &format!("{}/frequency.dat", fon);

        let fw = File::create(fon).unwrap();

        bincode::serialize_into(fw, &fa).unwrap();

        let fr = File::open(fon).unwrap();

        // Deserialize from the file.
        let mut fad = bincode::deserialize_from(fr).unwrap();

        // Assert that the data of both databases are equal.
        assert_eq!(fa, fad);
        // not writable
        assert_eq!(fad.insert(String::from("")), Err(analyze::ReadOnlyError));

        fad.load_dict(&dict);
        // now it should be writable
        assert_eq!(fad.insert(String::from("")), Ok( () ));
	}

    #[test]
    /// Test bzip2 multistream
    fn bzip2_read () {

        use bzip2::bufread::{BzDecoder, MultiBzDecoder};

        let dat = File::open(format!("{}.bz2", DBDATA)).unwrap();
        let ind = File::open(format!("{}.bz2", DBINDEX)).unwrap();

        let dat = BufReader::new(dat);
        let ind = BufReader::new(ind);

        let mut dat = MultiBzDecoder::new(dat);
        let mut ind = BzDecoder::new(ind);

        let mut buf = String::from("");

        ind.read_to_string(&mut buf).unwrap();

        debug!("Read index: {}", buf);
    }
}
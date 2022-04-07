mod test {
    use crate::*;
	use std::io::Read;
    use log::{debug};

	#[test]
    /// Test database serialization and deserialization.
	fn database_serialize_deserialize() {
        let mut db = File::open(format!("{}.bz2", DBDATA)).unwrap();

        // db.seek(SeekFrom::Start(ind[ARTICLEID].0)).unwrap();

        // db.seek(SeekFrom::Start(2369839923)).unwrap();

        let db = BufReader::new(db);
        let db = MultiBzDecoder::new(db);
        // let db = File::open(DBDATA).unwrap();
        let db = BufReader::new(db);
        let db = Database::new(db);


         
        let mut a = db.into_iter();

        let dict = load_dict("data/words").unwrap();

        let mut fa = database::frequency::Frequency::new("results/frequency.dat", &dict).unwrap();

        let mut c = 0;
        while let Some(e) = a.next() {
            fa.insert(e.unwrap().text).unwrap();

            c += 1;
            info!(target: "app::basic", "Parsed article {}", c);

            if c % 1000 == 0 {
                println!("Parsed article {}", c);
            }

            // if c > 100_000 { break; }
        }

        let fon = "results";
        std::fs::create_dir_all(fon).unwrap();
        let fon = &format!("{}/frequency-index.dat", fon);

        let fw = BufWriter::new(File::create(fon).unwrap());

        bincode::serialize_into(fw, &fa).unwrap();

        let fr = File::open(fon).unwrap();

        // Deserialize from the file.
        let mut fad = bincode::deserialize_from(fr).unwrap();

        // Assert that the data of both databases are equal.
        assert_eq!(fa, fad);
        // not writable
        assert_eq!(
            fad.insert(String::from("")),
            Err(database::error::ErrorKind::ReadOnly.into())
        );

        fad.set_dict(&dict);
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

        let dat = MultiBzDecoder::new(dat);
        let mut ind = BzDecoder::new(ind);

        let mut buf = String::from("");

        ind.read_to_string(&mut buf).unwrap();

        debug!("Read index: {}", buf);
    }

    #[test]
    fn index_read () {
        let indexmap: Regex = Regex::new(r"^(\d+):(\d+):(.+)$").unwrap();

        let ind = File::open(format!("{}.bz2", DBINDEX)).unwrap();
        let ind = BufReader::new(ind);
        let ind = BzDecoder::new(ind);
        let ind = BufReader::new(ind);

        let ind: Vec<(u64, u32, String)> = ind.lines()
            .map(|e| e.unwrap())
            .map(|e| {
                let s = indexmap.captures_iter(&e).next().unwrap();

                (
                    s[1].parse::<u64>().unwrap(), 
                    s[2].parse::<u32>().unwrap(), 
                    s[3].to_owned()
                )
            })
            .collect();

        const ARTICLEID: usize = 921235 + 70611;

        println!("Parsing starting at article {} id {}, {:?}, byte {}", ARTICLEID, ind[ARTICLEID].1, ind[ARTICLEID].2, ind[ARTICLEID].0);
    }
}
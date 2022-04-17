mod test {
    use bzip2::bufread::BzDecoder;
    use regex::Regex;
    use crate::*;
	use std::io::{BufRead, Read};
    
	#[test]
    /// Test database serialization and deserialization.
	fn database_serialize_deserialize() {
        let db = File::open(format!("{}.bz2", DBDATA)).unwrap();

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
            Err(database::error::ErrorKind::MissingDict.into())
        );

        fad.set_dict(&dict);
        // now it should be writable
        assert_eq!(fad.insert(String::from("")), Ok( () ));
	}

    #[test]
    /// Load the index file into memory.
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

    #[test]
    /// Test parse_wiki_text
    fn wikitext () {
        let mut tmp = File::open("tmp.log").unwrap();
        let mut contents = String::new();
        tmp.read_to_string(&mut contents).unwrap();

        println!("{:?}", database::read::CONFIG.parse(&contents));
    }

    #[test]
    /// Correlation test
    fn corr () {
        let dict = load_dict("data/words").unwrap();
        let dat: HashMap<u32,Vec<(u32,u16)>> = HashMap::from([
            (0, vec![(0, 1), (1, 1), (9, 2)]),
            (1, vec![(0, 1), (1, 1), (9, 2)]),
            (2, vec![(9, 1)]),
            (3, vec![(5, 1)])
        ]);

        let mut c = Correlation::new(dat, 10, "results/_test/corr.dat", &dict).unwrap();

        println!("{:?}", c.dict());

        println!("{:?}", c.corr("A","A's"));

        assert!(
            (c.corr("A","A's").unwrap_or(0.) - 1.).abs()
            < EPSILON
        );

        assert!(
            (c.corr("AMD","A").unwrap_or(0.) - 0.804030252207).abs()
            < EPSILON
        );

        assert!(
            (c.corr("AMD's","AMD").unwrap_or(0.) - -0.111111111111).abs()
            < EPSILON
        );
    }
}
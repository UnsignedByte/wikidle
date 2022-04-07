use crate::database::read::Database;
use std::io::{BufWriter, BufRead, BufReader, Seek, SeekFrom, Read};
use bzip2::bufread::{MultiBzDecoder, BzDecoder};
use std::fs::File;
use std::collections::HashSet;
use log::{info,debug};
use regex::{Regex, Captures};
use const_format::formatcp;
use database::frequency::*;

mod database;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let indexmap: Regex = Regex::new(r"^(\d+):(\d+):(.+)$").unwrap();

    let mut db = File::open(format!("{}.bz2", DBDATA)).unwrap();

    // let mut tmp = File::open("tmp.log").unwrap();
    // let mut contents = String::new();
    // tmp.read_to_string(&mut contents).unwrap();

    // info!("{:?}", database::read::CONFIG.parse(&contents));

    // let ind = File::open(DBINDEX).unwrap();
    // let ind = File::open(format!("{}.bz2", DBINDEX)).unwrap();
    // let ind = BufReader::new(ind);
    // let ind = BzDecoder::new(ind);
    // let ind = BufReader::new(ind);

    // let ind: Vec<(u64, u32, String)> = ind.lines()
    //     .map(|e| e.unwrap())
    //     .map(|e| {
    //         let s = indexmap.captures_iter(&e).next().unwrap();

    //         (
    //             s[1].parse::<u64>().unwrap(), 
    //             s[2].parse::<u32>().unwrap(), 
    //             s[3].to_owned()
    //         )
    //     })
    //     .collect();

    // const ARTICLEID: usize = 865000;

    // info!("Parsing starting at article {}, byte {}", ARTICLEID, ind[ARTICLEID].0);

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
        fa.insert(e.unwrap()).unwrap();

        c += 1;
        info!(target: "app::basic", "Parsed article {}", c);

        if c % 1000 == 0 {
            println!("Parsed article {}", c);
        }

        // if c > 100_000 { break; }
    }

    let fw = "results";
    std::fs::create_dir_all(fw).unwrap();
    let fw = &format!("{}/frequency-index.dat", fw);

    let fw = BufWriter::new(File::create(fw).unwrap());

    bincode::serialize_into(fw, &fa).unwrap();
}


#[cfg(test)]
mod test;
use crate::database::read::Database;
use std::io::{BufWriter, BufRead, BufReader};
use bzip2::read::{MultiBzDecoder};
use std::fs::File;
use std::collections::HashSet;
use log::{info,debug};
use const_format::formatcp;
use database::frequency::*;

mod database;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let db = File::open(format!("{}.bz2", DBDATA)).unwrap();
    // let db = BufReader::new(db);
    let db = MultiBzDecoder::new(db);
    let db = BufReader::new(db);
    let db = Database::new(db);
     
    let mut a = db.into_iter();

    let dict = load_dict("data/words").unwrap();

    let mut fa = database::frequency::Frequency::new(String::from("results/frequency.dat"), &dict).unwrap();

    let mut c = 0;
    while let Some(e) = a.next() {
        fa.insert(e.unwrap()).unwrap();

        c += 1;
        info!(target: "app::basic", "Parsed article {}", c);

        if c > 100_000 { break; }
    }

    let fw = "results";
    std::fs::create_dir_all(fw).unwrap();
    let fw = &format!("{}/frequency-index.dat", fw);

    let fw = BufWriter::new(File::create(fw).unwrap());

    bincode::serialize_into(fw, &fa).unwrap();
}


#[cfg(test)]
mod test;
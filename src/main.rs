use crate::database::read::Database;
use std::io::{BufWriter, BufRead, BufReader, Seek, SeekFrom};
use bzip2::bufread::{MultiBzDecoder, BzDecoder};
use std::fs::File;

use log::{info};
use regex::{Regex};
use const_format::formatcp;
use database::frequency::*;

mod database;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn gen_word_frequency<'a> (namespace: &str, dict: &'a Dict, start: u64) -> Frequency<'a>{
    let mut db = File::open(format!("{}.bz2", DBDATA)).unwrap();

    db.seek(SeekFrom::Start(start)).unwrap();

    let db = BufReader::new(db);
    let db = MultiBzDecoder::new(db);
    // let db = File::open(DBDATA).unwrap();
    let db = BufReader::new(db);
    let db = Database::new(db);


     
    let mut a = db.into_iter();

    let path = format!("results/{namespace}");
    std::fs::create_dir_all(&path).unwrap();

    let mut fa = database::frequency::Frequency::new(&format!("{path}/data.dat"), &dict).unwrap();

    while let Some(e) = a.next() {
        let page = e.unwrap();
        fa.insert(page.text).unwrap();

        info!(target: "app::basic", "Parsed article {}: {}", page.id, page.title);
    }

    let fw = &format!("{path}/index.dat");

    let fw = BufWriter::new(File::create(fw).unwrap());

    bincode::serialize_into(fw, &fa).unwrap();

    fa
}

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let dict = load_dict("data/words").unwrap();

    let fa = gen_word_frequency("frequency", &dict, 0);
}


#[cfg(test)]
mod test;
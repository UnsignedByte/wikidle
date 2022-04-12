use crate::database::read::Database;
use std::io::{BufWriter, BufReader, Seek, SeekFrom};
use bzip2::bufread::{MultiBzDecoder};
use std::fs::File;

use log::{info, error};
use const_format::formatcp;
use database::frequency::*;

mod database;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
// const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn gen_word_frequency<'a> (namespace: &str, dict: &'a Dict, start: u64) -> Frequency<'a>{
    let path = format!("results/{namespace}");

    let root = format!("{path}/index.dat");

    match File::open(&root) {
        Ok(f) => {
            info!("Loading database.");
            bincode::deserialize_from(f).unwrap()
        }
        Err(_) => {
            info!("Failed to read database, creating new instead.");

            let mut db = File::open(format!("{}.bz2", DBDATA)).unwrap();

            db.seek(SeekFrom::Start(start)).unwrap();

            let db = BufReader::new(db);
            let db = MultiBzDecoder::new(db);
            // let db = File::open(DBDATA).unwrap();
            let db = BufReader::new(db);
            let db = Database::new(db);

            let mut a = db.into_iter();

            std::fs::create_dir_all(&path).unwrap();

            let mut fa = database::frequency::Frequency::new(&format!("{path}/data.dat"), &dict).unwrap();

            while let Some(e) = a.next() {
                let page = match e {
                    Ok(x) => x,
                    Err(x) => {
                        error!("Failed to parse article with error {x:?}, skipped");
                        continue;
                    }
                };
                fa.insert(page.text).unwrap();

                info!(target: "app::basic", "Parsed article {}: {}", page.id, page.title);
            }

            let fw = BufWriter::new(File::create(&root).unwrap());

            bincode::serialize_into(fw, &fa).unwrap();

            fa
        }
    }
}

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let dict = load_dict("data/words").unwrap();

    let fa = gen_word_frequency("frequency", &dict, 0);
}


#[cfg(test)]
mod test;
use std::collections::HashMap;
use crate::database::{
    read::{Dict, load_dict, Database},
    correlation::Correlation,
    frequency::{Frequency}
};
use std::io::{BufWriter, BufReader, Seek, SeekFrom};
use bzip2::bufread::{MultiBzDecoder};
use std::fs::File;

use log::{info, error};
use const_format::formatcp;

mod database;

const DBNAME: &str = "enwiki-20220101-pages-articles-multistream";
const DBDATA: &str = formatcp!("data/{}/{0}.xml", DBNAME);
const DBINDEX: &str = formatcp!("data/{}/{0}-index.txt", DBNAME);

fn gen_word_frequency<'a> (namespace: &str, dict: &'a Dict, start: u64) -> Correlation{
    let path = format!("results/{namespace}");

    let root = format!("{path}/index.dat");
    let raw = format!("{path}/rawfreq.dat"); // raw frequency data
    let cpath = format!("{path}/corr.dat");

    // if let Ok(f) = File::open(&cpath) {
    //     info!("Correlation data already exists, loading.");
    //     return bincode::deserialize_from(f).unwrap();
    // }

    let dat: (HashMap<u32, Vec<(u32, u16)>>, usize) = match File::open(&raw) {
        Ok(f) => {
            info!("Directly loading raw data...");

            match bincode::deserialize_from(f) {
                Ok(dat) => dat,
                Err(_) => panic!("Invalid raw data.")
            }
        },
        Err(_) => {
            info!("Missing raw data, creating.");

            let mut fa = match File::open(&root) {
                Ok(f) => {
                    info!("Loading database.");
                    match bincode::deserialize_from(f) {
                        Ok(d) => d,
                        Err(e) => panic!("Failed to deserialize database with error:\n{e}")
                    }
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

                    let mut fa = Frequency::new(&format!("{path}/data.dat"), &dict).unwrap();

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
            };
            info!("Loading freq data to memory");
            let dat = (fa.load().unwrap(), fa.len());

            let fw = BufWriter::new(File::create(&raw).unwrap());
            bincode::serialize_into(fw, &dat);

            dat
        }
    };

    Correlation::new(&dat.0, dat.1, &cpath, &dict).unwrap()
}

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let dict = load_dict("data/words").unwrap();

    let corr = gen_word_frequency("frequency", &dict, 0);
}


#[cfg(test)]
mod test;
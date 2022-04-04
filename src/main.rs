use std::io::{BufRead, BufReader};
use std::fs::File;
use std::collections::HashSet;
use log::{info};

mod database;
mod analyze;

fn main() {
    log4rs::init_file("log/config.yaml", Default::default()).unwrap();

    info!("Initiated Logger");

    let s = "enwiki-20220101-pages-articles-multistream";

    let s = format!("data/{}/{0}.xml", s);
    let db = database::Database::new(&s).unwrap();
     
    let mut a = db.into_iter();

    let mut dict: HashSet<String> = HashSet::new();

    let df = "data/words";
    let df = File::open(df).expect(&format!("Missing {:?} file.", df));
    let df = BufReader::new(df);

    for l in df.lines() {
        dict.insert(l.unwrap().to_lowercase());
    }


    let mut fa = analyze::Frequency::new(&dict);

    let mut c = 0;
    while let Some(e) = a.next() {
        fa.insert(e.unwrap()).unwrap();

        c += 1;
        info!(target: "app::basic", "Parsed article {}", c);

        if c > 100_000 { break; }
    }

    let fw = "results";
    std::fs::create_dir_all(fw).unwrap();
    let fw = &format!("{}/frequency.dat", fw);

    let fw = File::create(fw).unwrap();

    bincode::serialize_into(fw, &fa).unwrap();
}


#[cfg(test)]
mod test;
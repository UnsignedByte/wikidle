use std::io::{BufRead, BufReader};
use std::fs::File;
use std::collections::HashSet;

mod database;
mod analyze;

fn main() {
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

    fa.insert(a.next().unwrap().unwrap());
    
    fa.insert(a.next().unwrap().unwrap());

    dbg!(fa);
}


#[cfg(test)]
mod test;
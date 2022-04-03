mod database;

fn main() {
    let s = "enwiki-20220101-pages-articles-multistream";

    let s = format!("data/{}/{0}.xml", s);
    let db = database::Database::new(&s).unwrap();

    let mut a = db.into_iter();

    let mut x = String::from("Wiki:\n\n");

    for _ in 0..2 {
        x += a.next().unwrap().unwrap().as_str();
    }

    println!("{}", x);
}


#[cfg(test)]
mod test;
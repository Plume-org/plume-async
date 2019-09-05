extern crate clap;
extern crate diesel;
extern crate dotenv;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate plume_models;
extern crate rpassword;

use clap::App;
use diesel::Connection;
use plume_models::{instance::Instance, Connection as Conn, CONFIG};
use std::io::{self, prelude::*};

mod instance;
mod migration;
mod search;
mod users;

fn main() {
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();

    let mut app = App::new("Plume CLI")
        .bin_name("plm")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Collection of tools to manage your Plume instance.")
        .subcommand(instance::command())
        .subcommand(migration::command())
        .subcommand(search::command())
        .subcommand(users::command());
    let matches = app.clone().get_matches();

    match dotenv::dotenv() {
        Ok(path) => info!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => error!("no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
    let conn = Conn::establish(CONFIG.database_url.as_str());
    let _ = conn.as_ref().map(|conn| Instance::cache_local(conn));

    match matches.subcommand() {
        ("instance", Some(args)) => {
            instance::run(args, &conn.expect("Couldn't connect to the database."))
        }
        ("migration", Some(args)) => {
            migration::run(args, &conn.expect("Couldn't connect to the database."))
        }
        ("search", Some(args)) => {
            search::run(args, &conn.expect("Couldn't connect to the database."))
        }
        ("users", Some(args)) => {
            users::run(args, &conn.expect("Couldn't connect to the database."))
        }
        _ => app.print_help().expect("Couldn't print help"),
    };
}

pub fn ask_for(something: &str) -> String {
    print!("{}: ", something);
    io::stdout().flush().expect("Couldn't flush STDOUT");
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Unable to read line");
    input.retain(|c| c != '\n');
    input
}

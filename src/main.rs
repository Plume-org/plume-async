#![allow(clippy::too_many_arguments)]
#![feature(decl_macro, proc_macro_hygiene, try_trait)]

extern crate activitypub;
extern crate actix_files;
extern crate actix_web;
extern crate askama_escape;
extern crate atom_syndication;
extern crate chrono;
extern crate clap;
extern crate colored;
extern crate ctrlc;
extern crate diesel;
extern crate dotenv;
#[macro_use]
extern crate gettext_macros;
extern crate gettext_utils;
extern crate guid_create;
extern crate heck;
extern crate lettre;
extern crate lettre_email;
extern crate multipart;
extern crate num_cpus;
extern crate plume_api;
extern crate plume_common;
extern crate plume_models;
#[macro_use]
extern crate runtime_fmt;
extern crate scheduled_thread_pool;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate serde_qs;
extern crate validator;
#[macro_use]
extern crate validator_derive;
extern crate webfinger;

use actix_web::{web as aweb, App as ActixApp, HttpResponse, HttpServer};
use clap::App;
use diesel::r2d2::ConnectionManager;
use plume_models::{
    db_conn::{DbPool, PragmaForeignKey},
    instance::Instance,
    migrations::IMPORTED_MIGRATIONS,
    search::{Searcher as UnmanagedSearcher, SearcherError},
    Connection, Error,
};
use scheduled_thread_pool::ScheduledThreadPool;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Duration;

init_i18n!(
    "plume", ar, bg, ca, cs, de, en, eo, es, fr, gl, hi, hr, it, ja, nb, pl, pt, ro, ru, sr, sk, sv
);

mod api;
mod mail;
#[cfg(feature = "test")]
mod test_routes;
mod web;

compile_i18n!();

fn main() -> std::io::Result<()> {
    App::new("Plume")
        .bin_name("plume")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Plume backend server")
        .after_help(
            r#"
The plume command should be run inside the directory
containing the `.env` configuration file and `static` directory.
See https://docs.joinplu.me/installation/config
and https://docs.joinplu.me/installation/init for more info.
        "#,
        )
        .get_matches();

    ctrlc::set_handler(move || {
        exit(0);
    })
    .expect("Error setting Ctrl-c handler");

    let mail = mail::init();

    HttpServer::new(|| ActixApp::new().service(
        aweb::scope("/")
            .service(api::service())
            .service(web::service())
            .service(
                // TODO: caching and co.
                actix_files::Files::new("/static", "./static")
            )
            .default_service(
                // TODO: real error page
                aweb::route().to(|| HttpResponse::NotFound())
            )
    ))
    .bind("127.0.0.1:7878")?
    .run()?;

    println!("Welcome to the async experiment of Plume.");
    println!("If you can only see this message, we're not done yet.");

    Ok(())
}

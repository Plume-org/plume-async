use actix_web::{web, Scope};

mod posts;

pub fn service() -> Scope {
    web::scope("/api/v1/").service(posts::service())
}

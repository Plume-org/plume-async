use actix_web::{web, Scope};

pub fn service() -> Scope {
	web::scope("/api/v1/")
}

use actix_web::{web, Scope, HttpResponse};

pub fn service() -> Scope {
    web::scope("/")
        .route("/test", web::get().to(|pool: web::Data<plume_models::db_conn::DbPool>| {
            let instance = plume_models::instance::Instance::get(&pool.get().unwrap(), 1).unwrap();
            HttpResponse::Ok().body(format!("Welcome on {}\n\n{}", instance.name, instance.short_description))
        }))
}

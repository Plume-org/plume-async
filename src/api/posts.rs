use actix_web::{web, HttpResponse, Scope};

pub fn service() -> Scope {
    web::scope("/posts/")
        .service(
            web::resource("/")
                .route(web::get().to(list))
                .route(web::post().to(create)),
        )
        .service(
            web::resource("/{id}")
                .route(web::get().to(details))
                .route(web::put().to(update))
                .route(web::delete().to(delete)),
        )
}

fn list() -> HttpResponse {
    unimplemented!()
}

fn create() -> HttpResponse {
    unimplemented!()
}

fn details() -> HttpResponse {
    unimplemented!()
}

fn update() -> HttpResponse {
    unimplemented!()
}

fn delete() -> HttpResponse {
    unimplemented!()
}

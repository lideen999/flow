use crate::controllers;
use axum::routing::{get, post};
use axum::Router;

pub fn routes() -> Router {
    Router::new()
        .merge(health_check_routes())
        .merge(connectors_routes())
        .merge(connector_images_routes())
}

fn health_check_routes() -> Router {
    Router::new().route("/health_check", get(controllers::health_check::show))
}

fn connectors_routes() -> Router {
    Router::new()
        .route(
            "/connectors",
            get(controllers::connectors::index).post(controllers::connectors::create),
        )
        .route(
            "/connectors/:connector_id/connector_images",
            get(controllers::connectors::images),
        )
}

fn connector_images_routes() -> Router {
    Router::new()
        .route(
            "/connector_images",
            get(controllers::connector_images::index).post(controllers::connector_images::create),
        )
        .route(
            "/connector_images/:image_id",
            get(controllers::connector_images::show),
        )
        .route(
            "/connector_images/:image_id/spec",
            get(controllers::connector_images::spec),
        )
        .route(
            "/connector_images/:image_id/discovery",
            post(controllers::connector_images::discovery),
        )
}

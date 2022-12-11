use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use axum::{extract::Extension, middleware, routing::get, Router, Server};

use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

use std::future::ready;
use std::net::SocketAddr;
use dotenv::dotenv;

use tokio::signal;

mod routes;
mod model;
mod observability;

use crate::routes::{graphql_handler, graphql_playground, health};
use crate::observability::metrics::{create_prometheus_recorder, track_metrics};
use crate::observability::tracing::create_tracer_from_env;
use crate::model::QueryRoot;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();
    let registry = Registry::default()
            .with(tracing_subscriber::fmt::layer().pretty());
    
    match create_tracer_from_env() {
        Some(tracer) => registry
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()
            .expect("Failed to register tracer with registry."),
        None => registry
            .try_init()
            .expect("Failed to register tracer with registry."),
    }

    info!("Server starting");

    let app = create_app(schema);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

fn create_app(schema: Schema<QueryRoot, EmptyMutation, EmptySubscription>) -> Router {
    let prometheus_recorder = create_prometheus_recorder();

    Router::new()
        .route("/health", get(health))
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/metrics", get(move || ready(prometheus_recorder.render())))
        .route_layer(middleware::from_fn(track_metrics))
        .layer(Extension(schema))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    opentelemetry::global::shutdown_tracer_provider();
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::routes::Health;
    use axum::{
        body::Body,
        http::{
            Method,
            Request,
        },
        response::Response
    };
    use tower::ServiceExt;

    fn test_create_app() -> Router {
        Router::new()
            .route("/health", get(health))
    }

    fn get_req_with_empty(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap()
    }

    async fn res_health(res: Response) -> Health {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let health: Health = serde_json::from_str(&body).unwrap();

        health
    }

    #[tokio::test]
    async fn health_check() {
        let expected = Health {
            healthy: true
        };

        let req = get_req_with_empty(Method::GET, "/health");
        let res = test_create_app().oneshot(req).await.unwrap();
        let health = res_health(res).await;

        assert_eq!(expected, health);
    }
}
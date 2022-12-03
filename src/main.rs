use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use axum::{extract::Extension, routing::get, Router, Server};
use std::net::SocketAddr;

mod routes;
mod model;

use crate::routes::{graphql_handler, graphql_playground, health};
use crate::model::QueryRoot;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();

    let app = create_app(schema);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn create_app(schema: Schema<QueryRoot, EmptyMutation, EmptySubscription>) -> Router {
    Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/health", get(health))
        .layer(Extension(schema))
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
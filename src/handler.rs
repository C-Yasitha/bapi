use std::fmt::Display;

use async_graphql::{http::GraphQLPlaygroundConfig,http::playground_source, Request as GraphQlRequest, Response as GraphQlResponse, ServerError as GraphQlError};
use http::{Method, StatusCode};
use lambda_http::{Body, Error, Request, RequestExt, Response};

use crate::{
    errors::{ClientError, ServerError},
    schema::SCHEMA,
};

pub async fn handle_request(request: Request) -> Result<Response<Body>, Error> {
    match (request.method(), request.uri().path()) {
        // Serve the GraphQL Playground at /playground
        (&Method::GET, "/playground") => {
            // Configure the playground to use your GraphQL endpoint
            let config = GraphQLPlaygroundConfig::new("http://localhost:9000/graphql");
            let html = playground_source(config);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/html")
                .body(Body::Text(html))
                .expect("Failed to render response"))
        },
        // Handle GraphQL queries and mutations at /graphql
        (&Method::POST, "/graphql") | (&Method::GET, "/graphql") => {
            let query = if request.method() == Method::POST {
                graphql_request_from_post(request)
            } else {
                graphql_request_from_get(request)
            };

            let query = match query {
                Err(e) => return error_response(StatusCode::BAD_REQUEST, graphql_error(e)),
                Ok(query) => query,
            };

            let response_body =
                serde_json::to_string(&SCHEMA.execute(query).await).map_err(ServerError::from)?;
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::Text(response_body))
                .map_err(ServerError::from)
                .map_err(Error::from)
        },
        // Default response for unsupported methods or paths
        _ => return Err(ClientError::MethodNotAllowed.into()),
    }
}

fn graphql_error(message: impl Display) -> String {
    let message = format!("{}", message);
    let response = GraphQlResponse::from_errors(vec![GraphQlError::new(message, None)]);
    serde_json::to_string(&response).expect("Valid response should never fail to serialize")
}

fn error_response(status: StatusCode, body: String) -> Result<Response<Body>, Error> {
    Ok(Response::builder().status(status).body(Body::Text(body))?)
}

fn graphql_request_from_post(request: Request) -> Result<GraphQlRequest, ClientError> {
    match request.into_body() {
        Body::Empty => Err(ClientError::EmptyBody),
        Body::Text(text) => serde_json::from_str::<GraphQlRequest>(&text).map_err(ClientError::from),
        Body::Binary(binary) => serde_json::from_slice::<GraphQlRequest>(&binary).map_err(ClientError::from)
    }
}

fn graphql_request_from_get(request: Request) -> Result<GraphQlRequest, ClientError> {
    let params = request.query_string_parameters();
    let query = params.first("query").ok_or(ClientError::MissingQuery)?;
    let mut request = async_graphql::Request::new(query);
    if let Some(operation_name) = params.first("operationName") {
        request = request.operation_name(operation_name);
    }
    if let Some(variables) = params.first("variables") {
        let value = serde_json::from_str(variables).unwrap_or_default();
        let variables = async_graphql::Variables::from_json(value);
        request = request.variables(variables);
    }
    Ok(request)
}

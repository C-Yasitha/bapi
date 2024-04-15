use std::fmt::Display;

use async_graphql::{
    http::playground_source, http::GraphQLPlaygroundConfig, Request as GraphQlRequest,
    Response as GraphQlResponse, ServerError as GraphQlError, UploadValue,
};
use http::{Method, StatusCode};
use lambda_http::{Body, Error, Request, RequestExt, Response};
use multer::Multipart;
use std::collections::HashMap;
use std::io::Cursor;
use tokio_util::codec::{BytesCodec, FramedRead};

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
        }
        // Handle GraphQL queries and mutations at /graphql
        (&Method::POST, "/graphql") | (&Method::GET, "/graphql") => {
            let query = if request.method() == Method::POST {
                graphql_request_from_post(request).await
            } else {
                graphql_request_from_get(request).await
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
        }
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

async fn graphql_request_from_post(request: Request) -> Result<GraphQlRequest, Error> {
    // Clone the headers before consuming the request body
    let headers = request.headers().clone();

    match request.into_body() {
        Body::Empty => Err(ClientError::EmptyBody.into()),
        Body::Text(text) => {
            serde_json::from_str::<GraphQlRequest>(&text).map_err(|e| ServerError::from(e).into())
        }
        Body::Binary(binary) => {
            let content_type = headers
                .get("Content-Type")
                .ok_or(ClientError::MissingContentType)?;

            let is_multipart = content_type
                .to_str()
                .map_err(|_| ClientError::InvalidData)?
                .starts_with("multipart/form-data");

            if is_multipart {
                let boundary = content_type
                    .to_str()?
                    .split(";")
                    .find(|s| s.trim_start().starts_with("boundary="))
                    .ok_or(ClientError::InvalidData)?
                    .split('=')
                    .nth(1)
                    .ok_or(ClientError::InvalidData)?
                    .trim();

                let mut multipart = Multipart::new(
                    FramedRead::new(Cursor::new(binary), BytesCodec::new()),
                    boundary.to_string(),
                );

                let mut operations: Option<GraphQlRequest> = None;
                let mut file_map = HashMap::new();
                let mut uploads = HashMap::new();

                while let Some(field) = multipart.next_field().await? {
                    match field.name() {
                        Some("operations") => {
                            let data = field.text().await?;
                            //print!("Operations: {}", data);
                            operations = serde_json::from_str::<GraphQlRequest>(&data).ok();
                        }
                        Some("map") => {
                            let data = field.text().await?;
                            //  print!(" map data {}",data);
                            file_map = serde_json::from_str::<HashMap<String, Vec<String>>>(&data)
                                .unwrap_or_default();
                        }
                        Some(file_field) if file_map.contains_key(file_field) => {
                            let filenames = file_map.get(file_field).unwrap();

                            let filename = field
                                .file_name()
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| "default_filename".to_string());
                            let content_type = field.content_type().map(|ctype| ctype.to_string());

                            let content = field.bytes().await?;

                            let upload = UploadValue {
                                filename: filename.clone(),
                                content_type: content_type,
                                content: content,
                            };

                            // uploads.push(upload);
                            uploads.insert(filenames.clone(), upload);
                        }
                        _ => {}
                    }
                }

                if let Some(mut op) = operations {
                    // op.uploads = uploads;

                    for (name, upload) in uploads {
                        op.set_upload(&name[0], upload);
                    }

                    //println valiables
                   // println!("{:?}", op.variables);
                    Ok(op)
                } else {
                    Err(Error::from(ClientError::MissingQuery))
                }
            } else {
                serde_json::from_slice::<GraphQlRequest>(&binary)
                    .map_err(|e| ServerError::from(e).into())
            }
        }
    }
}

async fn graphql_request_from_get(request: Request) -> Result<GraphQlRequest, Error> {
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

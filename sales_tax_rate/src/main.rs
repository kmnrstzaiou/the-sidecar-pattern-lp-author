use std::{convert::Infallible, net::SocketAddr};

use csv::Reader;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use serde_json::{json, Value};

/// This is our service handler. It receives a Request, routes on its
/// path, and returns a Future of a Response.
async fn handle_request(req: Request<Body>) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // Serve some instructions at /
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Try POSTing data to /find_rate such as: `curl http://localhost:8001/get_rate -XPOST -d '78701'`",
        ))),

        (&Method::POST, "/find_rate") => {
            let byte_stream = hyper::body::to_bytes(req).await?;
            let json: Value = serde_json::from_slice(&byte_stream)?;
            let zip = json["zip"].as_str().unwrap();

            let client = dapr::Dapr::new(3501);
            match client.get_state("statestore", zip).await? {
                Value::String(rate) => Ok(Response::new(Body::from(rate))),
                _ => {
                    let mut not_found = Response::new(Body::from("Not Found"));
                    *not_found.status_mut() = StatusCode::NOT_FOUND;
                    Ok(not_found)
                }
            }
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = dapr::Dapr::new(3501);
    let rates_data: &[u8] = include_bytes!("rates_by_zipcode.csv");
    let mut rdr = Reader::from_reader(rates_data);
    for result in rdr.records() {
        let record = result?;
        let kvs = json!([{
            "key": record[0], "value": record[1]
        }]);
        client.save_state("statestore", kvs).await?;
    }
    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    let make_svc = make_service_fn(|_| async move {
        Ok::<_, Infallible>(service_fn(move |req| handle_request(req)))
    });
    let server = Server::bind(&addr).serve(make_svc);
    dbg!("Server started on port 8001");
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
    Ok(())
}

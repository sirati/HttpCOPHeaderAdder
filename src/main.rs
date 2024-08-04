use hyper::{service::{make_service_fn, service_fn}, Body, Client, Request, Response, Server, Uri};
use hyper::client::HttpConnector;
use hyper::header::{HeaderValue, HeaderName};
use hyper_tungstenite::{tungstenite::Message, is_upgrade_request, upgrade};
use futures_util::stream::StreamExt;
use std::convert::Infallible;
use std::net::SocketAddr;
use futures_util::SinkExt;

async fn handle_http_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let client = Client::new();

    let new_uri = format!("http://127.0.0.1:8080{}", req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("")).parse::<Uri>().unwrap();
    let (mut parts, body) = req.into_parts();
    parts.uri = new_uri;
    let new_req = Request::from_parts(parts, body);

    let mut response = client.request(new_req).await?;
    response.headers_mut().insert("Cross-Origin-Opener-Policy", HeaderValue::from_static("same-origin"));
    response.headers_mut().insert("Cross-Origin-Embedder-Policy", HeaderValue::from_static("require-corp"));

    Ok(response)
}

async fn handle_websocket_connection(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let request_url = format!("ws://127.0.0.1:8080{}", req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or(""));
    println!("Handling WebSocket request for: {}", request_url);

    let (response, websocket) = match upgrade(req, None) {
        Ok((response, websocket)) => (response, websocket),
        Err(_) => return Ok(Response::builder().status(400).body(Body::from("WebSocket upgrade required")).unwrap()),
    };

    tokio::spawn(async move {
        let websocket = websocket.await.expect("WebSocket upgrade failed");

        let (server_ws, _) = tokio_tungstenite::connect_async(request_url)
            .await
            .expect("Failed to connect to WebSocket server");

        let (mut client_sender, mut client_receiver) = websocket.split();
        let (mut server_sender, mut server_receiver) = server_ws.split();

        let client_to_server = async {
            while let Some(message) = client_receiver.next().await {
                match message {
                    Ok(message) => {
                        if let Err(e) = server_sender.send(message).await {
                            eprintln!("Error forwarding message to server: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving message from client: {}", e);
                        break;
                    }
                }
            }

            // Close server connection properly if client connection is closed
            if let Err(e) = server_sender.close().await {
                eprintln!("Error closing connection to server: {}", e);
            }
        };

        let server_to_client = async {
            while let Some(message) = server_receiver.next().await {
                match message {
                    Ok(message) => {
                        if let Err(e) = client_sender.send(message).await {
                            eprintln!("Error forwarding message to client: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving message from server: {}", e);
                        break;
                    }
                }
            }

            // Close client connection properly if server connection is closed
            if let Err(e) = client_sender.close().await {
                eprintln!("Error closing connection to client: {}", e);
            }
        };

        tokio::select! {
            _ = client_to_server => (),
            _ = server_to_client => (),
        }
    });

    Ok(response)
}



async fn proxy_service(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    println!("Proxying request to: {}", req.uri());
    if is_upgrade_request(&req) {
        match handle_websocket_connection(req).await {
            Ok(resp) => Ok(resp),
            Err(_) => Ok(Response::builder().status(500).body(Body::from("WebSocket error")).unwrap()),
        }
    } else {
        match handle_http_request(req).await {
            Ok(resp) => Ok(resp),
            Err(_) => Ok(Response::builder().status(500).body(Body::from("Internal Server Error")).unwrap()),
        }
    }
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8081));
    let make_svc = make_service_fn(|_conn| {
        async { Ok::<_, Infallible>(service_fn(proxy_service)) }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

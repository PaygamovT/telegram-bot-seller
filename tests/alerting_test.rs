use axum::{routing::get, Router};
use std::sync::{Arc, Mutex};
use telegram_bot_seller::shared::alerting::send_with_retry;

#[tokio::test]
async fn test_send_with_retry_transient_failures() {
    let call_count = Arc::new(Mutex::new(0));
    let call_count_clone = call_count.clone();

    // Create a local axum router that fails 2 times and succeeds on the 3rd time
    let app = Router::new().route(
        "/test",
        get(move || {
            let mut count = call_count_clone.lock().unwrap();
            *count += 1;
            let current = *count;
            
            async move {
                if current < 3 {
                    axum::http::Response::builder()
                        .status(500)
                        .body("Transient Error".to_string())
                        .unwrap()
                } else {
                    axum::http::Response::builder()
                        .status(200)
                        .body("Success".to_string())
                        .unwrap()
                }
            }
        }),
    );

    // Bind to an ephemeral port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Make the request using send_with_retry
    let client = reqwest::Client::new();
    let req_builder = client.get(format!("http://127.0.0.1:{}/test", addr.port()));

    let res = send_with_retry(req_builder, 3).await.expect("Request should eventually succeed");
    assert_eq!(res.status(), 200);
    assert_eq!(*call_count.lock().unwrap(), 3);
}

#[tokio::test]
async fn test_send_with_retry_exhaust_retries() {
    let call_count = Arc::new(Mutex::new(0));
    let call_count_clone = call_count.clone();

    // Always fails with 503 Service Unavailable
    let app = Router::new().route(
        "/fail",
        get(move || {
            let mut count = call_count_clone.lock().unwrap();
            *count += 1;
            async move {
                axum::http::Response::builder()
                    .status(503)
                    .body("Always Fails".to_string())
                    .unwrap()
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let req_builder = client.get(format!("http://127.0.0.1:{}/fail", addr.port()));

    let res = send_with_retry(req_builder, 2).await.expect("Should return last failure response");
    assert_eq!(res.status(), 503);
    assert_eq!(*call_count.lock().unwrap(), 2);
}

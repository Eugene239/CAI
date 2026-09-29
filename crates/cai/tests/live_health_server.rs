use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::test]
async fn serves_health_over_a_bound_loopback_socket() {
    let listener = cai::server::bind_loopback("127.0.0.1:0".parse().expect("address"))
        .await
        .expect("listener must bind");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(cai::server::serve(listener));

    let mut connection = TcpStream::connect(address).await.expect("must connect");
    connection
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("request must write");
    let mut response = Vec::new();
    connection
        .read_to_end(&mut response)
        .await
        .expect("response must read");

    server.abort();

    let response = String::from_utf8(response).expect("response must be UTF-8");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: application/json\r\n"));
    assert!(response.ends_with("{\"status\":\"ok\"}"));
}

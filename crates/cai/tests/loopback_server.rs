use std::net::SocketAddr;

#[tokio::test]
async fn binds_only_to_a_loopback_listener() {
    let listener =
        cai::server::bind_loopback("127.0.0.1:0".parse::<SocketAddr>().expect("address"))
            .await
            .expect("loopback listener must bind");

    assert!(
        listener
            .local_addr()
            .expect("listener address")
            .ip()
            .is_loopback()
    );
}

#[tokio::test]
async fn rejects_a_non_loopback_listener_address() {
    let error = cai::server::bind_loopback("0.0.0.0:8080".parse::<SocketAddr>().expect("address"))
        .await
        .expect_err("non-loopback listener must fail");

    assert!(error.to_string().contains("loopback"));
}

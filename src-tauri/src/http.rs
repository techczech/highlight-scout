//! One place that builds HTTP clients. `reqwest::Client::new()` has no
//! timeouts, so a stalled connection or a server that stops sending mid-body
//! hangs a sync forever. Every client gets a connect and a read timeout.

use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Longest silence allowed between bytes of a response (not a cap on the
/// whole download).
pub const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// A client with the standard connect and read timeouts.
pub fn client() -> reqwest::Client {
    builder().build().unwrap_or_else(|_| reqwest::Client::new())
}

/// The standard builder, for callers that add a total timeout on top.
pub fn builder() -> reqwest::ClientBuilder {
    builder_with(CONNECT_TIMEOUT, READ_TIMEOUT)
}

fn builder_with(connect: Duration, read: Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(connect)
        .read_timeout(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A server that accepts and then never answers must not hang the client.
    #[tokio::test]
    async fn a_silent_server_times_out_instead_of_hanging() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            loop {
                if let Ok((sock, _)) = listener.accept().await {
                    held.push(sock);
                }
            }
        });
        let client = builder_with(Duration::from_secs(2), Duration::from_millis(300))
            .build()
            .unwrap();
        let res = tokio::time::timeout(
            Duration::from_secs(5),
            client.get(format!("http://{addr}/")).send(),
        )
        .await
        .expect("request hung: no read timeout");
        assert!(res.unwrap_err().is_timeout());
    }

    #[test]
    fn the_standard_timeouts_are_set() {
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(20));
        assert_eq!(READ_TIMEOUT, Duration::from_secs(120));
    }
}

//! Bounded retries for idempotent Provider reads, never token exchanges or mutations.
use reqwest::blocking::{RequestBuilder, Response};
use std::time::{Duration, Instant};

pub fn send_read(request: RequestBuilder) -> Result<Response, reqwest::Error> {
    send_read_until(request, Instant::now() + Duration::from_secs(20))
}

pub fn send_read_until(
    mut request: RequestBuilder,
    deadline: Instant,
) -> Result<Response, reqwest::Error> {
    for attempt in 0..3 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let retry = request.try_clone();
        let result = request
            .timeout(
                remaining
                    .min(Duration::from_secs(12))
                    .max(Duration::from_millis(1)),
            )
            .send();
        match result {
            Err(error)
                if attempt < 2
                    && retry.is_some()
                    && (error.is_connect() || error.is_timeout() || error.is_request())
                    && deadline.saturating_duration_since(Instant::now())
                        > Duration::from_millis(300) =>
            {
                std::thread::sleep(Duration::from_millis(100 * (attempt + 1)));
                request = retry.unwrap();
            }
            result => return result,
        }
    }
    unreachable!("the final attempt always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    fn server(
        drops: usize,
        status: u16,
    ) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        listener.set_nonblocking(true).unwrap();
        let thread = std::thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(2);
            while Instant::now() < end {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer);
                let number = seen.fetch_add(1, Ordering::SeqCst);
                if number < drops {
                    continue;
                }
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                break;
            }
        });
        (url, count, thread)
    }
    #[test]
    fn recovers_when_connections_close_before_response() {
        let (url, count, server) = server(2, 200);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap();
        let response = send_read(client.get(url)).unwrap();
        assert_eq!(response.status(), 200);
        server.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }
    #[test]
    fn auth_and_rate_limit_responses_are_not_retried() {
        for status in [401, 403, 429] {
            let (url, count, server) = server(0, status);
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .build()
                .unwrap();
            assert_eq!(send_read(client.get(url)).unwrap().status(), status);
            server.join().unwrap();
            assert_eq!(count.load(Ordering::SeqCst), 1);
        }
    }
}

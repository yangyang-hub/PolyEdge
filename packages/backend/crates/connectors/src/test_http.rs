use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

#[derive(Debug)]
pub(crate) struct CapturedHttpRequest {
    pub(crate) request_line: String,
    pub(crate) body: Value,
}

pub(crate) async fn spawn_json_response_server(
    response_body: &'static str,
) -> (String, oneshot::Receiver<CapturedHttpRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test http listener");
    let base_url = format!("http://{}", listener.local_addr().expect("test http addr"));
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept test http request");
        let mut buffer = Vec::new();
        let mut temp = [0u8; 4096];
        let mut header_end = None;
        let mut content_length = 0usize;

        loop {
            let read = socket
                .read(&mut temp)
                .await
                .expect("read test http request");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..read]);
            if header_end.is_none() {
                if let Some(index) = find_header_end(&buffer) {
                    header_end = Some(index);
                    let headers = String::from_utf8_lossy(&buffer[..index]);
                    content_length = parse_content_length(&headers);
                }
            }
            if let Some(index) = header_end {
                if buffer.len() >= index + 4 + content_length {
                    break;
                }
            }
        }

        let header_end = header_end.expect("test http request headers");
        let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
        let request_line = headers
            .lines()
            .next()
            .expect("test http request line")
            .to_string();
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        let body = serde_json::from_slice(&buffer[body_start..body_end])
            .expect("parse test http request json body");
        let _ = tx.send(CapturedHttpRequest { request_line, body });

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("write test http response");
    });

    (base_url, rx)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

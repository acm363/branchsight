use std::io::{Read, Write};
use std::net::{Ipv6Addr, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Method, Request, Response, Server};

use crate::api::{self, PING};
use crate::config::Config;

pub const PAGE_HEADER: &str = "X-BranchSight";

type BindError = Box<dyn std::error::Error + Send + Sync>;

pub fn listen(port: u16) -> Result<(Server, Option<Server>), BindError> {
    let ipv4 = Server::http(("127.0.0.1", port))?;
    let ipv6 = Server::http((Ipv6Addr::LOCALHOST, port)).ok();
    Ok((ipv4, ipv6))
}

pub fn serve(http: Server, config: Arc<Config>) {
    for request in http.incoming_requests() {
        let config = Arc::clone(&config);
        thread::spawn(move || handle(&config, request));
    }
}

pub fn is_running(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut body = String::new();
    stream.write_all(b"GET /api/ping HTTP/1.0\r\nHost: localhost\r\n\r\n").is_ok()
        && stream.read_to_string(&mut body).is_ok()
        && body.ends_with(PING)
}

fn handle(config: &Config, request: Request) {
    let header = |name: &'static str| request.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_owned());
    if !header("Host").is_some_and(|host| is_local_host(&host)) {
        let _ = request.respond(Response::from_string("forbidden host").with_status_code(403));
        return;
    }
    if api::needs_page(api::path(request.url())) && !is_own_page(header(PAGE_HEADER).as_deref(), header("Sec-Fetch-Site").as_deref()) {
        let _ = request.respond(Response::from_string("only the BranchSight page can call the API").with_status_code(403));
        return;
    }
    let reply = api::respond(config, *request.method() == Method::Post, request.url());
    let response = Response::from_string(reply.body)
        .with_status_code(reply.status)
        .with_header(header_line("Content-Type", reply.content_type))
        .with_header(header_line("Cache-Control", "no-store"));
    let _ = request.respond(response);
}

fn is_own_page(marker: Option<&str>, fetch_site: Option<&str>) -> bool {
    marker == Some("1") && fetch_site.is_none_or(|site| matches!(site, "same-origin" | "none"))
}

fn is_local_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let name = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.split(':').next().unwrap_or(""),
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1") || name.ends_with(".localhost")
}

fn header_line(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_hosts_only() {
        // Given.
        let local = ["localhost:7420", "127.0.0.1:7420", "[::1]:7420", "my-project.localhost:7420", "LOCALHOST"];
        let foreign = ["evil.com", "localhost.evil.com:7420", "127.0.0.2", "[::2]:7420", ""];

        // Then.
        assert!(local.iter().all(|h| is_local_host(h)));
        assert!(!foreign.iter().any(|h| is_local_host(h)));
    }

    #[test]
    fn the_api_needs_the_page_header_and_no_foreign_site() {
        // Then.
        assert!(is_own_page(Some("1"), None));
        assert!(is_own_page(Some("1"), Some("same-origin")));
        assert!(!is_own_page(None, None));
        assert!(!is_own_page(Some("1"), Some("cross-site")));
        assert!(!is_own_page(Some("1"), Some("same-site")));
    }
}

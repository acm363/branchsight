mod cli;

use std::process::exit;
use std::sync::Arc;
use std::thread;

use branchsight::{http, url};

fn main() {
    let cli::Args { port, config } = cli::parse();

    let (ipv4, ipv6) = match http::listen(port) {
        Ok(servers) => servers,
        Err(_) if http::is_running(port) => {
            match &config.default_repo {
                Some(top) => println!("http://localhost:{port}/?repo={}", url::encode(&top.to_string_lossy())),
                None => println!("http://localhost:{port}/"),
            }
            return;
        }
        Err(e) => {
            eprintln!("branchsight: cannot listen on 127.0.0.1:{port}: {e}");
            exit(1);
        }
    };
    if ipv6.is_none() {
        eprintln!("branchsight: no IPv6 loopback on [::1]:{port}, serving 127.0.0.1 only");
    }
    println!(
        "branchsight: http://localhost:{port}  default {}  roots {:?}",
        config.default_repo.as_ref().map_or("-".into(), |p| p.display().to_string()),
        config.roots
    );

    let config = Arc::new(config);
    if let Some(ipv6) = ipv6 {
        let config = Arc::clone(&config);
        thread::spawn(move || http::serve(ipv6, config));
    }
    http::serve(ipv4, config);
}

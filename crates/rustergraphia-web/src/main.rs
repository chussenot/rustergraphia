//! The rustergraphia-web server: the router of [`rustergraphia_web::app`],
//! configured from the environment (PORT, FRONT_DOOR_KEY, CHAPTER_DATE,
//! RUSTERGRAPHIA_PUBLIC).
//!
//! Subcommands mirror the chapter rake tasks: `chapter-check [days]`,
//! `chapter-runway`.

use rustergraphia_web::{app, chapter_check, Config};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("chapter-check") => {
            let days = args.get(2).and_then(|d| d.parse().ok()).unwrap_or(90);
            std::process::exit(chapter_check::check(days));
        }
        Some("chapter-runway") => {
            std::process::exit(chapter_check::runway());
        }
        _ => {}
    }
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9292);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("bind the server port");
    let bound = listener.local_addr().expect("a bound address");
    println!("rustergraphia-web listening on {bound}");
    axum::serve(listener, app(Config::from_env()))
        .await
        .expect("serve");
}

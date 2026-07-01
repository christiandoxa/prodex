//! Prodex control-plane entrypoint.
//!
//! This binary is intentionally a thin composition root placeholder while admin
//! HTTP/storage adapters are migrated behind boundary crates.

const HELP: &str = "prodex-control-plane\n\nControl-plane entrypoint.\n\nUSAGE:\n    prodex-control-plane --help\n    prodex-control-plane --version\n\nSTATUS:\n    The enterprise control-plane binary is present as a dedicated composition\n    root. Serving admin APIs is intentionally gated until adapters are wired to\n    prodex-application and prodex-control-plane.\n";

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help") | Some("-h") => {
            print!("{HELP}");
        }
        Some("--version") | Some("-V") => {
            println!("prodex-control-plane {}", env!("CARGO_PKG_VERSION"));
        }
        Some("serve") => {
            eprintln!(
                "prodex-control-plane serve is not wired yet; use the legacy `prodex gateway` admin path until control-plane adapter migration is complete"
            );
            std::process::exit(2);
        }
        Some(other) => {
            eprintln!("unknown prodex-control-plane argument: {other}\n\n{HELP}");
            std::process::exit(2);
        }
    }
}

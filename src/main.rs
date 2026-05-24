use dapctl::error::{ConfigError, DapError, ScanError};

fn main() {
    if let Err(e) = dapctl::cli::run() {
        eprintln!("error: {e:#}");
        let code = exit_code(&e);
        std::process::exit(code);
    }
}

fn exit_code(e: &anyhow::Error) -> i32 {
    for cause in e.chain() {
        if cause.downcast_ref::<ConfigError>().is_some() {
            return 2;
        }
        if cause.downcast_ref::<DapError>().is_some() {
            return 2;
        }
        if cause.downcast_ref::<ScanError>().is_some() {
            return 3;
        }
    }
    1
}

use std::io::{self, Read};
use std::process::ExitCode;

use winstrata_service::{Endpoint, Service};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("winstrata-service: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> io::Result<()> {
    let endpoint = Endpoint::user_default()?;
    let mut service = Service::listen_catalog(endpoint)?;
    eprintln!("listening {}", service.address());
    let mut byte = [0];
    let _ = io::stdin().read(&mut byte);
    service.shutdown();
    Ok(())
}

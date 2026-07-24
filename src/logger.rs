use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

pub fn init(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("kivi=debug")
    } else {
        EnvFilter::new("kivi=warn")
    };

    fmt()
        .with_target(false)
        .with_level(false)
        .without_time()
        .with_env_filter(filter)
        .init();
}

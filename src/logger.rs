use log::{Level, LevelFilter, Log, Metadata, Record};

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let prefix = match record.level() {
            Level::Error => console::style("ERROR").red().bold().to_string(),
            Level::Warn => console::style("WARN").yellow().bold().to_string(),
            Level::Info => console::style("INFO").blue().to_string(),
            Level::Debug => console::style("DEBUG").dim().to_string(),
            Level::Trace => console::style("TRACE").dim().to_string(),
        };
        eprintln!("{} {}", prefix, record.args());
    }

    fn flush(&self) {}
}

pub fn init(verbose: bool) {
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    };
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(level);
}

static LOGGER: Logger = Logger;

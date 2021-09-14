#![warn(clippy::all)]

use log::{debug, error, info, trace, warn};
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "tvrank")]
struct Opt {
  /// Verbose output (can be specified multiple times).
  #[structopt(short, long, parse(from_occurrences))]
  verbose: u8,

  /// Input directory.
  #[structopt(name = "DIR", parse(from_os_str))]
  dir: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
  let opt = Opt::from_args();

  let log_level = match opt.verbose {
    0 => log::LevelFilter::Off,
    1 => log::LevelFilter::Error,
    2 => log::LevelFilter::Warn,
    3 => log::LevelFilter::Info,
    4 => log::LevelFilter::Debug,
    _ => log::LevelFilter::Trace,
  };

  env_logger::Builder::new().filter_level(log_level).try_init().unwrap_or_else(|e| {
    eprintln!("Error initializing logger: {}", e);
  });

  error!("Error output enabled.");
  warn!("Warning output enabled.");
  info!("Info output enabled.");
  debug!("Debug output enabled.");
  trace!("Trace output enabled.");

  let dir = opt.dir.canonicalize()?;
  info!("Working on directory: {}", dir.display());

  Ok(())
}

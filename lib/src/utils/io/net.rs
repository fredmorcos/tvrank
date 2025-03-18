#![warn(clippy::all)]

//! Helpers for networking.

use std::io::{BufRead, BufReader};

use crate::utils::io::progress::ProgressPipe;

use flate2::bufread::GzDecoder;
use reqwest::Url;
use reqwest::blocking::{Client, Response};

/// Errors when doing networking.
#[derive(Debug, thiserror::Error)]
#[error("Networking error")]
pub enum Error {
  /// Networking error.
  #[error("Networking error: {0}")]
  Net(#[from] reqwest::Error),
}

/// Sends a GET request to the given URL and returns the response.
///
/// # Arguments
///
/// * `url` - The URL to send the GET request to.
pub fn get_response(url: Url) -> Result<Response, Error> {
  let client = Client::builder().build()?;
  let resp = client.get(url).send()?;
  Ok(resp)
}

/// Returns a reader for the given response.
///
/// # Arguments
///
/// * `resp` - Response returned for the GET request.
/// * `progress_fn` - Function to keep track of the download progress.
pub fn make_fetcher(resp: Response, progress_fn: impl Fn(u64)) -> impl BufRead {
  let progress = ProgressPipe::new(resp, progress_fn);
  let reader = BufReader::new(progress);
  let decoder = GzDecoder::new(reader);
  BufReader::new(decoder)
}

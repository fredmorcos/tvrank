#![warn(clippy::all)]

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub fn create_progress_bar(msg: String, len: u64) -> ProgressBar {
  let style = ProgressStyle::default_bar().template(
    "{msg}: {bar:40.cyan/blue} {percent:>3}%  {bytes:>9}/{total_bytes} {bytes_per_sec:>10} {elapsed:>4} ETA: {eta:>4}",
  );
  let bar = ProgressBar::new(len);
  let bar = if let Ok(style) = style {
    bar.with_style(style)
  } else {
    bar
  };

  bar.set_message(leak_string(msg));
  bar
}

pub fn create_progress_spinner(msg: String) -> ProgressBar {
  let style = ProgressStyle::default_spinner()
    .template("{msg}: {spinner:.cyan/blue}  {bytes:>9} {bytes_per_sec:>10} {elapsed:>4}");
  let bar = ProgressBar::new_spinner();
  let bar = if let Ok(style) = style {
    bar.with_style(style.tick_strings(&[r"◧", r"◩", r"⬒", r"⬔", r"◨", r"◪", r"⬓", r"⬕"]))
  } else {
    bar
  };

  bar.enable_steady_tick(Duration::from_millis(100));
  bar.set_message(leak_string(msg));
  bar
}

fn leak_string(s: String) -> &'static str {
  Box::leak(s.into_boxed_str())
}

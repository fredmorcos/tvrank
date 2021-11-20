use crate::utils::leak_string;
use indicatif::{ProgressBar, ProgressStyle};

pub fn create_progress_bar(msg: String, len: u64) -> ProgressBar {
  let bar =
    ProgressBar::new(len).with_style(ProgressStyle::default_bar().template(
          "{msg}: {bar:40.cyan/blue} {percent}%  {bytes}/{total_bytes} {bytes_per_sec} {elapsed} ETA: {eta}",
        ));

  bar.set_draw_rate(2);
  bar.set_message(leak_string(msg));
  bar
}

pub fn create_progress_spinner(msg: String) -> ProgressBar {
  let bar = ProgressBar::new_spinner().with_style(
    ProgressStyle::default_spinner()
      .template("{msg}: {spinner:.cyan/blue}  {bytes} {bytes_per_sec} {elapsed}")
      .tick_strings(&[r"◧", r"◩", r"⬒", r"⬔", r"◨", r"◪", r"⬓", r"⬕"]),
  );

  bar.set_draw_rate(2);
  bar.set_message(leak_string(msg));
  bar
}

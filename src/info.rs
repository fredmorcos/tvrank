#![warn(clippy::all)]

use derive_more::Display;
use log::warn;
use serde::{Deserialize, Deserializer};
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use tvrank::imdb::ImdbTitleId;
use tvrank::Res;

#[derive(Debug, Display)]
#[display(fmt = "")]
pub struct InfoErr;

impl Error for InfoErr {}

#[derive(Deserialize)]
pub struct TitleInfo {
  imdb: ImdbTitleInfo,
}

#[derive(Deserialize)]
pub struct ImdbTitleInfo {
  #[serde(deserialize_with = "deserialize_titleid")]
  id: ImdbTitleId<'static>,
}

fn deserialize_titleid<'de, D>(deserializer: D) -> Result<ImdbTitleId<'static>, D::Error>
where
  D: Deserializer<'de>,
{
  let s = Box::leak(String::deserialize(deserializer)?.into_boxed_str());
  let title_id = ImdbTitleId::try_from(s.as_bytes()).map_err(serde::de::Error::custom)?;
  Ok(title_id)
}

impl ImdbTitleInfo {
  pub fn id(&self) -> &ImdbTitleId<'static> {
    &self.id
  }
}

impl TitleInfo {
  pub fn from_path(path: &Path) -> Res<TitleInfo> {
    let title_info_path = path.join("tvrank.json");
    let title_info_file = fs::File::open(&title_info_path)?;
    let title_info_file_reader = BufReader::new(title_info_file);
    let title_info: Result<TitleInfo, _> = serde_json::from_reader(title_info_file_reader);

    let title_info = match title_info {
      Ok(title_info) => title_info,
      Err(err) => {
        warn!("Ignoring info in `{}` due to parse error: {}", title_info_path.display(), err);
        return Err(Box::new(InfoErr));
      }
    };

    Ok(title_info)
  }

  pub fn imdb(&self) -> &ImdbTitleInfo {
    &self.imdb
  }
}

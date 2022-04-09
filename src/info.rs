#![warn(clippy::all)]

use derive_more::Display;
use log::warn;
use serde::{Deserialize, Deserializer, Serialize};
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

#[derive(Serialize, Deserialize)]
pub struct ImdbTitleInfo<'a> {
  #[serde(deserialize_with = "deserialize_titleid")]
  id: ImdbTitleId<'a>,
}

impl<'a> ImdbTitleInfo<'a> {
  pub fn new(id: ImdbTitleId<'a>) -> Self {
    Self { id }
  }

  pub fn id(&self) -> &ImdbTitleId {
    &self.id
  }
}

fn deserialize_titleid<'a, 'de, D>(deserializer: D) -> Result<ImdbTitleId<'a>, D::Error>
where
  D: Deserializer<'de>,
{
  let s = Box::leak(String::deserialize(deserializer)?.into_boxed_str());
  let title_id = ImdbTitleId::try_from(s.as_bytes()).map_err(serde::de::Error::custom)?;
  Ok(title_id)
}

#[derive(Serialize, Deserialize)]
pub struct TitleInfo<'a> {
  imdb: ImdbTitleInfo<'a>,
}

impl<'a> TitleInfo<'a> {
  pub fn new(id: ImdbTitleId<'a>) -> Self {
    Self { imdb: ImdbTitleInfo::new(id) }
  }

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

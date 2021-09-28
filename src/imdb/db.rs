#![warn(clippy::all)]

use super::{
  error::Err,
  genre::{Genre, Genres},
  title::{Title, TitleType},
};
use crate::Res;
use atoi::atoi;
use derive_more::{Display, From};
use fnv::FnvHashMap;
use std::{
  io::{self, BufRead},
  ops::Index,
  str::FromStr,
};

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct TitleId(u64);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct MovieCookie(usize);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct SeriesCookie(usize);

type DbByYear<C> = FnvHashMap<Option<u16>, Vec<C>>;
type DbByName<C> = FnvHashMap<String, DbByYear<C>>;
type DbById<C> = FnvHashMap<TitleId, C>;

pub(crate) struct Db {
  /// Movies information.
  movies: Vec<Title>,
  /// Map from movie names to years to movies.
  movies_db: DbByName<MovieCookie>,
  /// Map from IMDB ID to movies.
  movies_ids: DbById<MovieCookie>,

  /// Series information.
  series: Vec<Title>,
  /// Map from series or episode names to years to series.
  series_db: DbByName<SeriesCookie>,
  /// Map from IMDB ID to series.
  series_ids: DbById<SeriesCookie>,
}

impl Index<&MovieCookie> for Db {
  type Output = Title;

  fn index(&self, index: &MovieCookie) -> &Self::Output {
    unsafe { self.movies.get_unchecked(index.0) }
  }
}

impl Index<&SeriesCookie> for Db {
  type Output = Title;

  fn index(&self, index: &SeriesCookie) -> &Self::Output {
    unsafe { self.series.get_unchecked(index.0) }
  }
}

impl Db {
  const TAB: u8 = b'\t';
  const ZERO: u8 = b'0';
  const ONE: u8 = b'1';
  const COMMA: u8 = b',';
  const NL: u8 = b'\n';

  const NOT_AVAIL: &'static [u8; 2] = b"\\N";

  fn skip_line<R: BufRead>(data: &mut io::Bytes<R>) -> Res<()> {
    for c in data {
      let c = c?;

      if c == Db::NL {
        break;
      }
    }

    Ok(())
  }

  fn parse_cell<R: BufRead>(data: &mut io::Bytes<R>, tok: &mut Vec<u8>) -> Res<()> {
    tok.clear();

    for c in data {
      let c = c?;

      if c == Db::TAB {
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() {
      Err::eof()
    } else {
      Ok(())
    }
  }

  fn parse_title<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<()> {
    tok.clear();
    res.clear();

    for c in data {
      let c = c?;

      if c == Db::TAB {
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() {
      Err::eof()
    } else {
      res.push_str(std::str::from_utf8(tok)?);
      Ok(())
    }
  }

  fn parse_is_adult<R: BufRead>(data: &mut io::Bytes<R>) -> Res<bool> {
    let is_adult = match Db::next_byte(data)? {
      Db::ZERO => false,
      Db::ONE => true,
      _ => return Err::adult(),
    };

    if Db::next_byte(data)? != Db::TAB {
      return Err::adult();
    }

    Ok(is_adult)
  }

  fn parse_genre<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<bool> {
    tok.clear();
    res.clear();

    let mut finish = false;

    for c in data {
      let c = c?;

      if c == Db::COMMA {
        break;
      } else if c == Db::NL {
        finish = true;
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() || tok == Self::NOT_AVAIL {
      Ok(true)
    } else {
      res.push_str(unsafe { std::str::from_utf8_unchecked(tok) });
      Ok(finish)
    }
  }

  fn parse_genres<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<Genres> {
    let mut genres = Genres::default();

    loop {
      let finish = Self::parse_genre(data, tok, res)?;

      if tok == Self::NOT_AVAIL {
        break;
      }

      let genre = Genre::from_str(res).map_err(|_| Err::Genre)?;
      genres.add_genre(genre);

      if finish {
        break;
      }
    }

    Ok(genres)
  }

  fn next_byte<R: BufRead>(data: &mut io::Bytes<R>) -> Res<u8> {
    if let Some(current) = data.next() {
      Ok(current?)
    } else {
      Err::eof()
    }
  }

  pub fn new<R: BufRead>(mut data: io::Bytes<R>) -> Res<Self> {
    let mut movies: Vec<Title> = Vec::new();
    let mut movies_db: DbByName<MovieCookie> = FnvHashMap::default();
    let mut movies_ids: DbById<MovieCookie> = FnvHashMap::default();

    let mut series: Vec<Title> = Vec::new();
    let mut series_db: DbByName<SeriesCookie> = FnvHashMap::default();
    let mut series_ids: DbById<SeriesCookie> = FnvHashMap::default();

    let mut tok = Vec::new();
    let mut res = String::new();

    let _ = Self::skip_line(&mut data)?;

    loop {
      let c = if let Some(c) = data.next() {
        c?
      } else {
        return Ok(Db { movies, movies_db, movies_ids, series, series_db, series_ids });
      };

      if c != b't' {
        return Err::id();
      }

      let c = Db::next_byte(&mut data)?;

      if c != b't' {
        return Err::id();
      }

      Db::parse_cell(&mut data, &mut tok)?;
      let id = atoi::<u64>(&tok).ok_or(Err::IdNumber)?;

      Db::parse_cell(&mut data, &mut tok)?;
      let title_type = unsafe { std::str::from_utf8_unchecked(&tok) };
      let title_type = TitleType::from_str(title_type).map_err(|_| Err::TitleType)?;

      let mut ptitle = String::new();
      Db::parse_title(&mut data, &mut tok, &mut ptitle)?;
      Db::parse_title(&mut data, &mut tok, &mut res)?;
      let otitle = if ptitle == res {
        None
      } else {
        let otitle = Some(res);
        res = String::new();
        otitle
      };

      let is_adult = Db::parse_is_adult(&mut data)?;

      Db::parse_cell(&mut data, &mut tok)?;
      let start_year = match tok.as_slice() {
        b"\\N" => None,
        start_year => Some(atoi::<u16>(start_year).ok_or(Err::StartYear)?),
      };

      Db::parse_cell(&mut data, &mut tok)?;
      let end_year = match tok.as_slice() {
        b"\\N" => None,
        end_year => Some(atoi::<u16>(end_year).ok_or(Err::EndYear)?),
      };

      Db::parse_cell(&mut data, &mut tok)?;
      let runtime_minutes = match tok.as_slice() {
        b"\\N" => None,
        runtime_minutes => Some(atoi::<u16>(runtime_minutes).ok_or(Err::RuntimeMinutes)?),
      };

      let genres = Db::parse_genres(&mut data, &mut tok, &mut res)?;

      fn update_db<T: From<usize> + Copy>(
        db: &mut DbByName<T>,
        cookie: T,
        title_name: String,
        year: Option<u16>,
      ) {
        db.entry(title_name)
          .and_modify(|by_year| {
            by_year
              .entry(year)
              .and_modify(|titles| titles.push(cookie))
              .or_insert_with(|| vec![cookie]);
          })
          .or_insert_with(|| {
            let mut by_year = FnvHashMap::default();
            by_year.insert(year, vec![cookie]);
            by_year
          });
      }

      let title_id = TitleId(id);
      let title =
        Title::new(title_type, is_adult, start_year, end_year, runtime_minutes, genres);

      if title_type.is_movie() {
        let cookie = MovieCookie::from(movies.len());
        movies.push(title);

        if movies_ids.insert(title_id, cookie).is_some() {
          return Err::duplicate(title_id.0);
        }

        update_db(&mut movies_db, cookie, ptitle, start_year);

        if let Some(otitle) = otitle {
          update_db(&mut movies_db, cookie, otitle, start_year);
        }
      } else if title_type.is_series() {
        let cookie = SeriesCookie::from(series.len());
        series.push(title);

        if series_ids.insert(title_id, cookie).is_some() {
          return Err::duplicate(title_id.0);
        }

        update_db(&mut series_db, cookie, ptitle, start_year);

        if let Some(otitle) = otitle {
          update_db(&mut series_db, cookie, otitle, start_year);
        }
      }
    }
  }

  pub(crate) fn lookup_movie(
    &self,
    title: &str,
    year: Option<u16>,
  ) -> Option<impl Iterator<Item = &Title>> {
    self.movies_db.get(title).and_then(move |by_year| {
      by_year
        .get(&year)
        .map(move |cookies| cookies.iter().map(move |cookie| &self[cookie]))
    })
  }
}

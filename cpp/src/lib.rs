use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::path::Path;
use tvrank::imdb::Imdb as ImdbService;

pub struct Service {
  imdb: ImdbService,
}

#[repr(C)]
pub enum ServiceError {
  InvalidCacheDirString,
  ErrorCreatingService,
}

/// Create a service object.
///
/// This will create and load the database files from the cache directory (usually
/// `~/.cache/tvrank`). If the database files don't already exist or are too old, they
/// will be downloaded and created. `progress_cb` and `data` are used to report progress
/// on the download of database files.
///
/// If `force_db_update` is `true`, the database files are re-downloaded and re-created
/// regardless of their age.
///
/// # Safety
///
/// This function potentially dereferences `cache_dir`.
#[no_mangle]
pub unsafe extern "C" fn tvrank_service_new(
  cache_dir: *const c_char,
  force_db_update: bool,
  progress_cb: extern "C" fn(*mut c_void, Option<&u64>, u64),
  data: *mut c_void,
  error: &mut ServiceError,
) -> *mut Service {
  let cache_dir = match CStr::from_ptr(cache_dir).to_str() {
    Ok(s) => s,
    Err(e) => {
      eprintln!("TVrank error: {e}");
      *error = ServiceError::InvalidCacheDirString;
      return std::ptr::null_mut();
    }
  };
  let cache_dir = Path::new(&cache_dir);
  let svc = match ImdbService::new(cache_dir, force_db_update, &|content_len, delta| {
    progress_cb(data, content_len.as_ref(), delta)
  }) {
    Ok(s) => s,
    Err(e) => {
      eprintln!("TVrank error: {e}");
      *error = ServiceError::ErrorCreatingService;
      return std::ptr::null_mut();
    }
  };
  Box::into_raw(Box::new(Service { imdb: svc }))
}

#[no_mangle]
pub extern "C" fn tvrank_service_entries_count(
  service: &Service,
  movies_count: &mut usize,
  series_count: &mut usize,
) {
  let (n_movies, n_series) = service.imdb.n_entries();
  *movies_count = n_movies;
  *series_count = n_series;
}

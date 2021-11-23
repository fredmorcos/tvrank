use titlecase::titlecase;

pub(crate) fn leak_string(s: String) -> &'static str {
  Box::leak(s.into_boxed_str())
}

pub(crate) fn humantitle(title: &[u8]) -> String {
  titlecase(unsafe { std::str::from_utf8_unchecked(title) })
}

pub fn leak_string(s: String) -> &'static str {
  Box::leak(s.into_boxed_str())
}

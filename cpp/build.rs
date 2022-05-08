use std::env;
use std::path::Path;

fn main() {
  let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

  let mangle_config = cbindgen::MangleConfig::default();
  let mangle_config = cbindgen::MangleConfig { remove_underscores: true, ..mangle_config };

  let export_config = cbindgen::ExportConfig::default();
  let export_config = cbindgen::ExportConfig {
    renaming_overrides_prefixing: false,
    prefix: Some(String::from("TVrank")),
    mangle: mangle_config,
    ..export_config
  };

  let config = cbindgen::Config::default();
  let config = cbindgen::Config { usize_is_size_t: true, export: export_config, ..config };

  let extern_c_header = r##"
#ifdef __cplusplus
extern "C" {
#endif // __cplusplus
  "##;

  let extern_c_footer = r##"
#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
  "##;

  cbindgen::Builder::new()
    .with_config(config)
    .with_language(cbindgen::Language::C)
    .with_std_types(true)
    .with_documentation(true)
    .with_style(cbindgen::Style::Both)
    .with_tab_width(2)
    .with_line_length(110)
    .with_pragma_once(true)
    .with_namespace("tvrank")
    .with_include_version(true)
    .with_header(extern_c_header)
    .with_trailer(extern_c_footer)
    .with_crate(&crate_dir)
    .generate()
    .unwrap()
    .write_to_file(Path::new(&crate_dir).join("..").join("target").join("tvrank.h"));
  println!("cargo:rerun-if-changed={}", Path::new(&crate_dir).join("src").join("lib.rs").display());
}

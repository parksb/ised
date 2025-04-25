use ised::app::App;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempdir::TempDir;

fn write_config(dir: &Path, content: &str) {
    let config_path = dir.join("ised.config.toml");
    let mut file = fs::File::create(config_path).unwrap();
    writeln!(file, "{}", content).unwrap();
}

#[test]
fn test_loads_glob_filter_from_config() {
    let tmp_dir = TempDir::new("ised_test_config").unwrap();
    let config_content = r#"
        [files]
        glob_filter = ["!**/.git/**", "*.rs"]
    "#;

    write_config(tmp_dir.path(), config_content);

    std::env::set_current_dir(tmp_dir.path()).unwrap();

    let app = App::new();

    assert_eq!(app.filter_input.trim(), "!**/.git/**,*.rs");
}

#[test]
fn test_no_config_file_defaults_to_empty() {
    let tmp_dir = TempDir::new("ised_test_empty").unwrap();
    std::env::set_current_dir(tmp_dir.path()).unwrap();

    let app = App::new();

    assert_eq!(app.filter_input.trim(), "");
}

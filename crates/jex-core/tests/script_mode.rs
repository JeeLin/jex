use jex_core::run::get_or_compile;
use jex_core::script::{parse_script, ScriptMeta};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_shebang_jbang_style() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "///usr/bin/env jex\n//DEPS com.google.code.gson:gson:2.11.0\n//JAVA 21\n\nimport com.google.gson.Gson;\n\npublic class Main {{}}"
    )
    .unwrap();

    let meta = parse_script(file.path()).unwrap();
    assert!(meta.is_script);
    assert_eq!(meta.deps.len(), 1);
    assert_eq!(meta.deps[0], "com.google.code.gson:gson:2.11.0");
    assert_eq!(meta.java_version.as_deref(), Some("21"));
}

#[test]
fn test_parse_shebang_hashbang_style() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "#!/usr/bin/env jex\n//DEPS org.apache.commons:commons-lang3:3.14.0\n\nimport org.apache.commons.lang3.StringUtils;\n\npublic class Main {{}}"
    )
    .unwrap();

    let meta = parse_script(file.path()).unwrap();
    assert!(meta.is_script);
    assert_eq!(meta.deps.len(), 1);
    assert_eq!(meta.deps[0], "org.apache.commons:commons-lang3:3.14.0");
}

#[test]
fn test_parse_multiple_deps() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "//DEPS com.google.code.gson:gson:2.11.0\n//DEPS org.apache.commons:commons-lang3:3.14.0\n//DEPS com.google.guava:guava:33.0.0-jre\n\nimport com.google.gson.Gson;\n\npublic class Main {{}}"
    )
    .unwrap();

    let meta = parse_script(file.path()).unwrap();
    assert!(meta.is_script);
    assert_eq!(meta.deps.len(), 3);
}

#[test]
fn test_non_script_file() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "import java.util.List;\n\npublic class Main {{}}").unwrap();

    let meta = parse_script(file.path()).unwrap();
    assert!(!meta.is_script);
}

#[test]
fn test_get_or_compile_cache_creation() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("test.java");
    std::fs::write(&script, "//DEPS a:b:1.0\npublic class Test {}").unwrap();
    let meta = ScriptMeta {
        java_version: None,
        deps: vec!["a:b:1.0".to_string()],
        is_script: true,
    };
    let result = get_or_compile(&script, &meta);
    assert!(result.is_ok());
    let class_dir = result.unwrap();
    assert!(class_dir.exists());
}

#[test]
#[ignore = "sandbox prevents writing to ~/.jex cache dir"]
fn test_get_or_compile_cache_hit() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("test.java");
    std::fs::write(&script, "//DEPS a:b:1.0\npublic class Test {}").unwrap();
    let meta = ScriptMeta {
        java_version: None,
        deps: vec!["a:b:1.0".to_string()],
        is_script: true,
    };
    // First call creates cache
    let dir1 = get_or_compile(&script, &meta).unwrap();
    // Create a .class file to simulate previous compilation
    std::fs::write(dir1.join("Test.class"), b"mock").unwrap();
    // Second call should hit cache
    let dir2 = get_or_compile(&script, &meta).unwrap();
    assert_eq!(dir1, dir2);
}

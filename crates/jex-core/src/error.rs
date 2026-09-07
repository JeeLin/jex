//! 错误体系
use std::fmt;

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<notify::Error> for Error {
    fn from(e: notify::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error(e.to_string())
    }
}
impl Error {
    pub fn new(msg: impl Into<String>) -> Self {
        Error(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_new() {
        let e = Error::new("test error");
        assert_eq!(e.0, "test error");
    }

    #[test]
    fn test_error_display() {
        let e = Error::new("display test");
        assert_eq!(format!("{}", e), "display test");
    }

    #[test]
    fn test_error_debug() {
        let e = Error::new("debug test");
        assert!(format!("{:?}", e).contains("debug test"));
    }

    #[test]
    fn test_error_std_error_trait() {
        let e = Error::new("trait test");
        let err: &dyn std::error::Error = &e;
        assert_eq!(err.to_string(), "trait test");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e: Error = io_err.into();
        assert!(e.0.contains("file not found"));
    }

    #[test]
    fn test_from_toml_de_error() {
        let toml_err = "invalid toml".parse::<toml::Value>().unwrap_err();
        let e: Error = toml_err.into();
        assert!(!e.0.is_empty());
    }

    #[test]
    fn test_from_toml_ser_error() {
        // 构造一个会导致序列化错误的场景
        // HashMap with non-string keys can't be serialized to TOML
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(1u32, "value");
        let e: Error = toml::to_string(&map).unwrap_err().into();
        assert!(!e.0.is_empty());
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let e: Error = json_err.into();
        assert!(!e.0.is_empty());
    }

    #[test]
    fn test_error_with_string() {
        let msg = "owned string".to_string();
        let e = Error::new(msg);
        assert_eq!(e.0, "owned string");
    }
}

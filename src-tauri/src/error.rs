use serde::Serialize;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new("network", message)
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self::new("busy", message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::new("auth", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::new("unknown", message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::new("unknown", message)
    }
}

impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.message
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::not_found(error.to_string()),
            std::io::ErrorKind::PermissionDenied => Self::new("permission", error.to_string()),
            std::io::ErrorKind::StorageFull => Self::new("disk_full", error.to_string()),
            _ => Self::new("io", error.to_string()),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() || error.is_connect() {
            Self::network(error.to_string())
        } else {
            Self::new("http", error.to_string())
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::invalid(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn a_bare_string_error_keeps_its_text_and_gets_the_unknown_code() {
        let error: AppError = "instance not found".to_string().into();
        assert_eq!(error.code, "unknown");
        assert_eq!(error.message, "instance not found");
    }

    #[test]
    fn io_errors_are_classified_so_the_ui_can_branch_on_them() {
        let missing: AppError =
            std::io::Error::new(std::io::ErrorKind::NotFound, "no such file").into();
        assert_eq!(missing.code, "not_found");

        let denied: AppError =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope").into();
        assert_eq!(denied.code, "permission");

        let other: AppError = std::io::Error::other("weird").into();
        assert_eq!(other.code, "io");
    }

    #[test]
    fn serializes_with_a_code_the_frontend_can_read() {
        let json = serde_json::to_string(&AppError::busy("already running")).unwrap();
        assert_eq!(json, r#"{"code":"busy","message":"already running"}"#);
    }
}

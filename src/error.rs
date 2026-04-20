//! Error types for the Crawlery web crawler.
//!
//! This module defines custom error types that can occur during web crawling operations.
//! All errors can be converted to `anyhow::Error` for flexible error handling.

use std::fmt;

/// Result type alias using anyhow::Error as the error type.
pub type Result<T> = anyhow::Result<T>;

/// Custom error types for the crawler.
#[derive(Debug)]
pub enum CrawlError {
    /// HTTP request failed
    HttpError {
        url: String,
        message: String,
    },

    /// Failed to parse HTML content
    ParseError {
        url: String,
        message: String,
    },

    /// Invalid URL format
    InvalidUrl {
        url: String,
        reason: String,
    },

    /// Browser automation error
    BrowserError {
        message: String,
    },

    /// Configuration error
    ConfigError {
        message: String,
    },

    /// File I/O error
    IoError {
        path: String,
        message: String,
    },

    /// Maximum depth exceeded
    MaxDepthExceeded {
        url: String,
        max_depth: usize,
    },

    /// Timeout error
    Timeout {
        url: String,
        duration_secs: u64,
    },

    /// Rate limit exceeded
    RateLimitExceeded {
        retry_after: Option<u64>,
    },

    /// Validation error
    ValidationError {
        field: String,
        message: String,
    },
}

impl fmt::Display for CrawlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrawlError::HttpError { url, message } => {
                write!(f, "HTTP error for URL '{}': {}", url, message)
            }
            CrawlError::ParseError { url, message } => {
                write!(f, "Parse error for URL '{}': {}", url, message)
            }
            CrawlError::InvalidUrl { url, reason } => {
                write!(f, "Invalid URL '{}': {}", url, reason)
            }
            CrawlError::BrowserError { message } => {
                write!(f, "Browser automation error: {}", message)
            }
            CrawlError::ConfigError { message } => {
                write!(f, "Configuration error: {}", message)
            }
            CrawlError::IoError { path, message } => {
                write!(f, "I/O error for path '{}': {}", path, message)
            }
            CrawlError::MaxDepthExceeded { url, max_depth } => {
                write!(
                    f,
                    "Maximum crawl depth {} exceeded for URL '{}'",
                    max_depth, url
                )
            }
            CrawlError::Timeout { url, duration_secs } => {
                write!(
                    f,
                    "Request timeout after {} seconds for URL '{}'",
                    duration_secs, url
                )
            }
            CrawlError::RateLimitExceeded { retry_after } => {
                if let Some(secs) = retry_after {
                    write!(f, "Rate limit exceeded. Retry after {} seconds", secs)
                } else {
                    write!(f, "Rate limit exceeded")
                }
            }
            CrawlError::ValidationError { field, message } => {
                write!(f, "Validation error for field '{}': {}", field, message)
            }
        }
    }
}

impl std::error::Error for CrawlError {}

/// Extension trait for adding context to errors.
pub trait ErrorContext<T> {
    /// Add context about the URL being processed.
    fn with_url_context(self, url: &str) -> Result<T>;

    /// Add context about the operation being performed.
    fn with_operation_context(self, operation: &str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_url_context(self, url: &str) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("Error processing URL '{}': {}", url, e))
    }

    fn with_operation_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("Error during '{}': {}", operation, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_error_display() {
        let err = CrawlError::HttpError {
            url: "https://example.com".to_string(),
            message: "404 Not Found".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "HTTP error for URL 'https://example.com': 404 Not Found"
        );
    }

    #[test]
    fn test_invalid_url_display() {
        let err = CrawlError::InvalidUrl {
            url: "not a url".to_string(),
            reason: "missing scheme".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid URL 'not a url': missing scheme"
        );
    }

    #[test]
    fn test_timeout_display() {
        let err = CrawlError::Timeout {
            url: "https://slow.example.com".to_string(),
            duration_secs: 30,
        };
        assert_eq!(
            err.to_string(),
            "Request timeout after 30 seconds for URL 'https://slow.example.com'"
        );
    }
}

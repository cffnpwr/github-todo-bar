use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum GithubError {
    #[error("authentication failed (HTTP 401), body: {body}")]
    Unauthorized {
        body: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("HTTP error: status {status}, body: {body}")]
    Http {
        status: u16,
        body: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("failed to connect to GitHub API")]
    Connection(#[from] reqwest::Error),

    #[error("failed to parse response")]
    Parse(#[from] serde_json::Error),

    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

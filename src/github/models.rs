use serde::Deserialize;
use url::Url;

use crate::github::error::GithubError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Issue {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: Url,
    pub(crate) repository: Repository,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PullRequest {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: Url,
    pub(crate) repository: Repository,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Repository {
    pub(crate) owner: String,
    pub(crate) name: String,
}

#[derive(Deserialize)]
pub(super) struct RawIssue {
    number: u64,
    title: String,
    html_url: Url,
    repository: RawRepository,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

impl RawIssue {
    pub(super) fn is_pull_request(&self) -> bool {
        self.pull_request.is_some()
    }
}

#[derive(Deserialize)]
struct RawRepository {
    owner: RawOwner,
    name: String,
}

#[derive(Deserialize)]
struct RawOwner {
    login: String,
}

#[derive(Deserialize)]
pub(super) struct RawSearchResults {
    pub(super) items: Vec<RawSearchItem>,
}

#[derive(Deserialize)]
pub(super) struct RawSearchItem {
    number: u64,
    title: String,
    html_url: Url,
    repository_url: String,
}

impl From<RawIssue> for Issue {
    fn from(raw: RawIssue) -> Self {
        Self {
            number: raw.number,
            title: raw.title,
            url: raw.html_url,
            repository: Repository {
                owner: raw.repository.owner.login,
                name: raw.repository.name,
            },
        }
    }
}

impl TryFrom<RawSearchItem> for PullRequest {
    type Error = GithubError;

    fn try_from(raw: RawSearchItem) -> Result<Self, Self::Error> {
        Ok(Self {
            number: raw.number,
            title: raw.title,
            url: raw.html_url,
            repository: parse_repository_url(&raw.repository_url)?,
        })
    }
}

fn parse_repository_url(url: &str) -> Result<Repository, GithubError> {
    let parsed = Url::parse(url).map_err(|e| {
        GithubError::InvalidResponse(format!("invalid repository_url '{url}': {e}"))
    })?;
    let segments: Vec<&str> = parsed
        .path_segments()
        .ok_or_else(|| GithubError::InvalidResponse(format!("missing path segments: {url}")))?
        .filter(|s| !s.is_empty())
        .collect();
    if segments.len() != 3 || segments[0] != "repos" {
        return Err(GithubError::InvalidResponse(format!(
            "unexpected repository_url path: {url}"
        )));
    }
    Ok(Repository {
        owner: segments[1].to_string(),
        name: segments[2].to_string(),
    })
}

use std::sync::Once;

use reqwest::{Client as HttpClient, Response, StatusCode};
use url::Url;

use crate::github::error::GithubError;
use crate::github::models::{Issue, PullRequest, RawIssue, RawSearchResults};

const DEFAULT_BASE_URL: &str = "https://api.github.com";
const ACCEPT_HEADER: &str = "application/vnd.github+json";
const API_VERSION_HEADER: &str = "2022-11-28";

static INIT_CRYPTO_PROVIDER: Once = Once::new();

fn ensure_crypto_provider() {
    INIT_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[allow(dead_code)]
pub(crate) struct GitHubClient {
    http: HttpClient,
    base_url: Url,
    token: String,
}

#[allow(dead_code)]
impl GitHubClient {
    pub(crate) fn new(token: String) -> Self {
        let base_url = Url::parse(DEFAULT_BASE_URL).expect("valid default base URL");
        Self::with_base_url(token, base_url)
    }

    pub(crate) fn with_base_url(token: String, base_url: Url) -> Self {
        ensure_crypto_provider();
        let http = HttpClient::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            base_url,
            token,
        }
    }

    pub(crate) async fn assigned_issues(&self) -> Result<Vec<Issue>, GithubError> {
        let mut url = self.endpoint("issues");
        url.query_pairs_mut()
            .append_pair("filter", "assigned")
            .append_pair("state", "open");
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", ACCEPT_HEADER)
            .header("X-GitHub-Api-Version", API_VERSION_HEADER)
            .send()
            .await?;
        let body = ensure_success(response).await?;
        let raw: Vec<RawIssue> = serde_json::from_str(&body)?;
        Ok(raw
            .into_iter()
            .filter(|item| !item.is_pull_request())
            .map(Issue::from)
            .collect())
    }

    pub(crate) async fn assigned_pull_requests(&self) -> Result<Vec<PullRequest>, GithubError> {
        let mut url = self.endpoint("search/issues");
        url.query_pairs_mut()
            .append_pair("q", "is:pr is:open assignee:@me");
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", ACCEPT_HEADER)
            .header("X-GitHub-Api-Version", API_VERSION_HEADER)
            .send()
            .await?;
        let body = ensure_success(response).await?;
        let raw: RawSearchResults = serde_json::from_str(&body)?;
        raw.items.into_iter().map(PullRequest::try_from).collect()
    }

    fn endpoint(&self, path: &str) -> Url {
        self.base_url.join(path).expect("valid endpoint path")
    }
}

async fn ensure_success(response: Response) -> Result<String, GithubError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response.text().await?);
    }
    let source = response
        .error_for_status_ref()
        .expect_err("non-success status must yield an error");
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED {
        Err(GithubError::Unauthorized { body, source })
    } else {
        Err(GithubError::Http {
            status: status.as_u16(),
            body,
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use const_format::concatcp;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::github::models::Repository;

    const TEST_TOKEN: &str = "test-pat-12345";
    const EXPECTED_USER_AGENT: &str =
        concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
    const EXPECTED_AUTH: &str = concatcp!("Bearer ", TEST_TOKEN);

    fn build_client(base_url: &str) -> GitHubClient {
        let url = Url::parse(base_url).expect("test base URL");
        GitHubClient::with_base_url(TEST_TOKEN.to_string(), url)
    }

    #[tokio::test]
    async fn positive_assigned_issues_returns_issues_excluding_prs() {
        // Arrange
        let server = MockServer::start().await;
        let body = json!([
            {
                "number": 1,
                "title": "first issue",
                "html_url": "https://github.com/octocat/hello-world/issues/1",
                "repository": {
                    "owner": { "login": "octocat" },
                    "name": "hello-world"
                }
            },
            {
                "number": 2,
                "title": "this PR should be filtered",
                "html_url": "https://github.com/octocat/hello-world/pull/2",
                "repository": {
                    "owner": { "login": "octocat" },
                    "name": "hello-world"
                },
                "pull_request": {
                    "url": "https://api.github.com/repos/octocat/hello-world/pulls/2"
                }
            },
            {
                "number": 3,
                "title": "Unicode タイトル 🚀 \"quoted\"",
                "html_url": "https://github.com/octocat/hello-world/issues/3",
                "repository": {
                    "owner": { "login": "octocat" },
                    "name": "hello-world"
                }
            }
        ]);
        Mock::given(method("GET"))
            .and(path("/issues"))
            .and(query_param("filter", "assigned"))
            .and(query_param("state", "open"))
            .and(header("Authorization", EXPECTED_AUTH))
            .and(header("User-Agent", EXPECTED_USER_AGENT))
            .and(header("Accept", "application/vnd.github+json"))
            .and(header("X-GitHub-Api-Version", "2022-11-28"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&server)
            .await;
        let client = build_client(&server.uri());
        let expected = vec![
            Issue {
                number: 1,
                title: "first issue".to_string(),
                url: Url::parse("https://github.com/octocat/hello-world/issues/1").unwrap(),
                repository: Repository {
                    owner: "octocat".into(),
                    name: "hello-world".into(),
                },
            },
            Issue {
                number: 3,
                title: "Unicode タイトル 🚀 \"quoted\"".to_string(),
                url: Url::parse("https://github.com/octocat/hello-world/issues/3").unwrap(),
                repository: Repository {
                    owner: "octocat".into(),
                    name: "hello-world".into(),
                },
            },
        ];

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    async fn positive_assigned_issues_returns_empty_vec_for_empty_response() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());
        let expected: Vec<Issue> = Vec::new();

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    async fn negative_assigned_issues_returns_unauthorized_on_401() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Unauthorized { .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_issues_returns_http_error_on_403() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 403, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_issues_returns_http_error_on_404() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 404, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_issues_returns_http_error_on_500() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 500, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_issues_returns_parse_error_on_invalid_json() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_issues().await;

        // Assert
        assert!(matches!(result.unwrap_err(), GithubError::Parse(_)));
    }

    #[tokio::test]
    async fn positive_assigned_pull_requests_returns_prs() {
        // Arrange
        let server = MockServer::start().await;
        let body = json!({
            "total_count": 2,
            "incomplete_results": false,
            "items": [
                {
                    "number": 10,
                    "title": "fix bug",
                    "html_url": "https://github.com/octocat/hello-world/pull/10",
                    "repository_url": "https://api.github.com/repos/octocat/hello-world"
                },
                {
                    "number": 11,
                    "title": "PR タイトル 🚀 \"quoted\"",
                    "html_url": "https://github.com/octocat/another-repo/pull/11",
                    "repository_url": "https://api.github.com/repos/octocat/another-repo"
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(query_param("q", "is:pr is:open assignee:@me"))
            .and(header("Authorization", EXPECTED_AUTH))
            .and(header("User-Agent", EXPECTED_USER_AGENT))
            .and(header("Accept", "application/vnd.github+json"))
            .and(header("X-GitHub-Api-Version", "2022-11-28"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&server)
            .await;
        let client = build_client(&server.uri());
        let expected = vec![
            PullRequest {
                number: 10,
                title: "fix bug".to_string(),
                url: Url::parse("https://github.com/octocat/hello-world/pull/10").unwrap(),
                repository: Repository {
                    owner: "octocat".into(),
                    name: "hello-world".into(),
                },
            },
            PullRequest {
                number: 11,
                title: "PR タイトル 🚀 \"quoted\"".to_string(),
                url: Url::parse("https://github.com/octocat/another-repo/pull/11").unwrap(),
                repository: Repository {
                    owner: "octocat".into(),
                    name: "another-repo".into(),
                },
            },
        ];

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    async fn positive_assigned_pull_requests_returns_empty_vec_for_empty_items() {
        // Arrange
        let server = MockServer::start().await;
        let body = json!({
            "total_count": 0,
            "incomplete_results": false,
            "items": []
        });
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());
        let expected: Vec<PullRequest> = Vec::new();

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_unauthorized_on_401() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Unauthorized { .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_http_error_on_403() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 403, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_http_error_on_404() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 404, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_http_error_on_500() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::Http { status: 500, .. }
        ));
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_parse_error_on_invalid_json() {
        // Arrange
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(result.unwrap_err(), GithubError::Parse(_)));
    }

    fn build_live_client() -> GitHubClient {
        let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
        GitHubClient::new(token)
    }

    #[tokio::test]
    #[ignore = "calls real GitHub API; requires GITHUB_TOKEN"]
    async fn positive_assigned_issues_live_returns_issues() {
        // Arrange
        let client = build_live_client();

        // Act
        let result = client.assigned_issues().await;

        // Assert
        let issues = result.unwrap();
        eprintln!("assigned issues: {} item(s)", issues.len());
        for issue in &issues {
            eprintln!(
                "  #{} [{}/{}] {} ({})",
                issue.number, issue.repository.owner, issue.repository.name, issue.title, issue.url
            );
        }
    }

    #[tokio::test]
    #[ignore = "calls real GitHub API; requires GITHUB_TOKEN"]
    async fn positive_assigned_pull_requests_live_returns_pull_requests() {
        // Arrange
        let client = build_live_client();

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        let prs = result.unwrap();
        eprintln!("assigned pull requests: {} item(s)", prs.len());
        for pr in &prs {
            eprintln!(
                "  #{} [{}/{}] {} ({})",
                pr.number, pr.repository.owner, pr.repository.name, pr.title, pr.url
            );
        }
    }

    #[tokio::test]
    async fn negative_assigned_pull_requests_returns_invalid_response_on_invalid_repository_url() {
        // Arrange
        let server = MockServer::start().await;
        let body = json!({
            "total_count": 1,
            "incomplete_results": false,
            "items": [
                {
                    "number": 99,
                    "title": "broken url",
                    "html_url": "https://github.com/octocat/hello-world/pull/99",
                    "repository_url": "https://example.com/foo"
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&server)
            .await;
        let client = build_client(&server.uri());

        // Act
        let result = client.assigned_pull_requests().await;

        // Assert
        assert!(matches!(
            result.unwrap_err(),
            GithubError::InvalidResponse(_)
        ));
    }
}

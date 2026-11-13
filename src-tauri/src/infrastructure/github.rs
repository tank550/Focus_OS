use serde::{Deserialize, Serialize};
use thiserror::Error;

const API_URL: &str = "https://api.github.com";
const PER_PAGE: usize = 100;
const MAX_PAGES: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub forks_count: u64,
    #[serde(default)]
    pub topics: Vec<String>,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct GithubApiRepository {
    id: u64,
    name: String,
    full_name: String,
    private: bool,
    html_url: String,
    description: Option<String>,
    default_branch: String,
    language: Option<String>,
    stargazers_count: u64,
    forks_count: u64,
    #[serde(default)]
    topics: Vec<String>,
    updated_at: String,
}

impl From<GithubApiRepository> for GithubRepository {
    fn from(repository: GithubApiRepository) -> Self {
        Self {
            id: repository.id,
            name: repository.name,
            full_name: repository.full_name,
            private: repository.private,
            html_url: repository.html_url,
            description: repository.description,
            default_branch: repository.default_branch,
            language: repository.language,
            stargazers_count: repository.stargazers_count,
            forks_count: repository.forks_count,
            topics: repository.topics,
            updated_at: repository.updated_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("GitHub token cannot be empty")]
    EmptyToken,
    #[error("GitHub returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("GitHub request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GitHub repository listing exceeded the safety pagination limit")]
    PaginationLimit,
}

pub struct GithubClient {
    client: reqwest::Client,
    token: String,
}

impl GithubClient {
    pub fn new(token: &str) -> Result<Self, GithubError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(GithubError::EmptyToken);
        }
        let client = reqwest::Client::builder()
            .user_agent("FocusOS/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            token: token.to_owned(),
        })
    }

    pub async fn list_owned_repositories(&self) -> Result<Vec<GithubRepository>, GithubError> {
        let mut repositories = Vec::new();
        for page in 1..=MAX_PAGES {
            let response = self
                .client
                .get(format!("{API_URL}/user/repos"))
                .bearer_auth(&self.token)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .query(&[
                    ("visibility", "all"),
                    ("affiliation", "owner"),
                    ("sort", "updated"),
                    ("direction", "desc"),
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(GithubError::HttpStatus(response.status().as_u16()));
            }
            let page_repositories = response.json::<Vec<GithubApiRepository>>().await?;
            let page_len = page_repositories.len();
            repositories.extend(page_repositories.into_iter().map(GithubRepository::from));
            if page_len < PER_PAGE {
                return Ok(repositories);
            }
        }
        Err(GithubError::PaginationLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_contract_parses_github_api_fields() {
        let repo = GithubRepository::from(
            serde_json::from_str::<GithubApiRepository>(
                r#"{
                "id": 42,
                "name": "focus-os",
                "full_name": "octocat/focus-os",
                "private": true,
                "html_url": "https://github.com/octocat/focus-os",
                "description": null,
                "default_branch": "main",
                "language": "Rust",
                "stargazers_count": 3,
                "forks_count": 1,
                "topics": ["productivity"],
                "updated_at": "2026-11-12T12:00:00Z"
            }"#,
            )
            .unwrap(),
        );

        assert_eq!(repo.full_name, "octocat/focus-os");
        assert!(repo.private);
        assert_eq!(repo.topics, ["productivity"]);
        assert_eq!(repo.language.as_deref(), Some("Rust"));
    }

    #[test]
    fn github_client_rejects_an_empty_token() {
        assert!(matches!(
            GithubClient::new(" \n "),
            Err(GithubError::EmptyToken)
        ));
    }
}

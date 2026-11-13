use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

const API_URL: &str = "https://api.github.com";
const PER_PAGE: usize = 100;
const MAX_PAGES: u32 = 100;
const MAX_TREE_BYTES: usize = 12 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 12;
const MAX_SOURCE_FILE_BYTES: u64 = 16 * 1024;
const MAX_SOURCE_BYTES: u64 = 96 * 1024;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubSourceSummary {
    pub full_name: String,
    pub default_branch: String,
    pub description: Option<String>,
    pub topics: Vec<String>,
    pub languages: BTreeMap<String, u64>,
    pub source_file_count: usize,
    pub test_file_count: usize,
    pub documentation_file_count: usize,
    pub manifest_files: Vec<String>,
    pub has_readme: bool,
    pub inspected_file_count: usize,
    pub inspected_lines: usize,
    pub declarations_found: usize,
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

#[derive(Deserialize)]
struct GithubTreeResponse {
    truncated: bool,
    tree: Vec<GithubTreeEntry>,
}

#[derive(Deserialize)]
struct GithubTreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct GithubBlob {
    content: String,
    encoding: String,
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
    #[error("GitHub returned invalid repository data: {0}")]
    InvalidResponse(#[from] serde_json::Error),
    #[error("GitHub repository listing exceeded the safety pagination limit")]
    PaginationLimit,
    #[error("The repository is not owned by the connected account")]
    RepositoryNotOwned,
    #[error("The repository name format is invalid")]
    InvalidRepositoryName,
    #[error("The GitHub repository tree is too large or truncated for safe inspection")]
    TreeTooLarge,
    #[error("The GitHub source response was not base64 encoded")]
    UnsupportedBlobEncoding,
    #[error("The GitHub source file could not be decoded")]
    InvalidBlob,
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
            let mut url = api_url(&["user", "repos"])?;
            url.query_pairs_mut()
                .append_pair("visibility", "all")
                .append_pair("affiliation", "owner")
                .append_pair("sort", "updated")
                .append_pair("direction", "desc")
                .append_pair("per_page", "100")
                .append_pair("page", &page.to_string());
            let response = self.get(url).await?;
            let page_repositories = response.json::<Vec<GithubApiRepository>>().await?;
            let page_len = page_repositories.len();
            repositories.extend(page_repositories.into_iter().map(GithubRepository::from));
            if page_len < PER_PAGE {
                return Ok(repositories);
            }
        }
        Err(GithubError::PaginationLimit)
    }

    pub async fn inspect_owned_repository(
        &self,
        full_name: &str,
    ) -> Result<GithubSourceSummary, GithubError> {
        let (owner, name) = split_repository_name(full_name)?;
        let repository = self
            .list_owned_repositories()
            .await?
            .into_iter()
            .find(|repository| repository.full_name == full_name)
            .ok_or(GithubError::RepositoryNotOwned)?;

        let languages = self
            .get(api_url(&["repos", owner, name, "languages"])?)
            .await?
            .json::<BTreeMap<String, u64>>()
            .await?;
        let mut tree_url = api_url(&[
            "repos",
            owner,
            name,
            "git",
            "trees",
            &repository.default_branch,
        ])?;
        tree_url.query_pairs_mut().append_pair("recursive", "1");
        let tree_response = self.get(tree_url).await?;
        if tree_response
            .content_length()
            .is_some_and(|length| length > MAX_TREE_BYTES as u64)
        {
            return Err(GithubError::TreeTooLarge);
        }
        let tree_bytes = tree_response.bytes().await?;
        if tree_bytes.len() > MAX_TREE_BYTES {
            return Err(GithubError::TreeTooLarge);
        }
        let tree = serde_json::from_slice::<GithubTreeResponse>(&tree_bytes)?;
        if tree.truncated {
            return Err(GithubError::TreeTooLarge);
        }

        let files = tree
            .tree
            .into_iter()
            .filter(|entry| entry.entry_type == "blob")
            .collect::<Vec<_>>();
        let source_file_count = files
            .iter()
            .filter(|entry| is_source_file(&entry.path))
            .count();
        let test_file_count = files
            .iter()
            .filter(|entry| is_test_file(&entry.path))
            .count();
        let documentation_file_count = files
            .iter()
            .filter(|entry| is_documentation_file(&entry.path))
            .count();
        let has_readme = files.iter().any(|entry| {
            entry
                .path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.to_ascii_lowercase().starts_with("readme"))
        });
        let mut manifest_files = files
            .iter()
            .filter(|entry| is_manifest_file(&entry.path))
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        manifest_files.sort();
        manifest_files.truncate(20);

        let mut candidates = files
            .into_iter()
            .filter(|entry| {
                is_source_file(&entry.path)
                    && entry
                        .size
                        .is_some_and(|size| size > 0 && size <= MAX_SOURCE_FILE_BYTES)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            source_priority(&left.path)
                .cmp(&source_priority(&right.path))
                .then_with(|| left.path.cmp(&right.path))
        });

        let mut inspected_file_count = 0;
        let mut inspected_lines = 0;
        let mut declarations_found = 0;
        let mut total_source_bytes = 0;
        for entry in candidates.into_iter().take(MAX_SOURCE_FILES) {
            let size = entry.size.unwrap_or_default();
            if total_source_bytes + size > MAX_SOURCE_BYTES {
                continue;
            }
            let blob = self
                .get(api_url(&[
                    "repos", owner, name, "git", "blobs", &entry.sha,
                ])?)
                .await?
                .json::<GithubBlob>()
                .await?;
            if blob.encoding != "base64" {
                return Err(GithubError::UnsupportedBlobEncoding);
            }
            let compact_content = blob
                .content
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect::<String>();
            let source = STANDARD
                .decode(compact_content)
                .map_err(|_| GithubError::InvalidBlob)?;
            let source = String::from_utf8_lossy(&source);
            let (lines, declarations) = code_metrics(&source);
            inspected_file_count += 1;
            inspected_lines += lines;
            declarations_found += declarations;
            total_source_bytes += size;
        }

        Ok(GithubSourceSummary {
            full_name: repository.full_name,
            default_branch: repository.default_branch,
            description: repository.description,
            topics: repository.topics,
            languages,
            source_file_count,
            test_file_count,
            documentation_file_count,
            manifest_files,
            has_readme,
            inspected_file_count,
            inspected_lines,
            declarations_found,
        })
    }

    async fn get(&self, url: reqwest::Url) -> Result<reqwest::Response, GithubError> {
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(GithubError::HttpStatus(response.status().as_u16()));
        }
        Ok(response)
    }
}

fn api_url(segments: &[&str]) -> Result<reqwest::Url, GithubError> {
    let mut url = reqwest::Url::parse(API_URL).map_err(|_| GithubError::InvalidRepositoryName)?;
    url.path_segments_mut()
        .map_err(|_| GithubError::InvalidRepositoryName)?
        .extend(segments.iter().copied());
    Ok(url)
}

fn split_repository_name(full_name: &str) -> Result<(&str, &str), GithubError> {
    let mut segments = full_name.split('/');
    let owner = segments.next().ok_or(GithubError::InvalidRepositoryName)?;
    let name = segments.next().ok_or(GithubError::InvalidRepositoryName)?;
    if segments.next().is_some()
        || ![owner, name].iter().all(|segment| {
            !segment.is_empty()
                && *segment != "."
                && *segment != ".."
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
    {
        return Err(GithubError::InvalidRepositoryName);
    }
    Ok((owner, name))
}

fn is_source_file(path: &str) -> bool {
    if is_ignored_path(path) {
        return false;
    }
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "c" | "cc"
                | "cpp"
                | "cs"
                | "go"
                | "h"
                | "hpp"
                | "java"
                | "js"
                | "jsx"
                | "kt"
                | "php"
                | "py"
                | "rb"
                | "rs"
                | "swift"
                | "ts"
                | "tsx"
        )
    })
}

fn is_ignored_path(path: &str) -> bool {
    path.split('/').any(|segment| {
        matches!(
            segment.to_ascii_lowercase().as_str(),
            ".git"
                | ".next"
                | ".venv"
                | "build"
                | "coverage"
                | "dist"
                | "generated"
                | "node_modules"
                | "target"
                | "third_party"
                | "vendor"
        )
    })
}

fn is_test_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/tests/")
        || lower.starts_with("tests/")
        || lower.contains("/test/")
        || lower.starts_with("test/")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.go")
}

fn is_documentation_file(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(extension.to_ascii_lowercase().as_str(), "md" | "rst")
    })
}

fn is_manifest_file(path: &str) -> bool {
    matches!(
        path.rsplit('/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "cargo.toml"
            | "composer.json"
            | "go.mod"
            | "package.json"
            | "pom.xml"
            | "pyproject.toml"
            | "requirements.txt"
    )
}

fn source_priority(path: &str) -> u8 {
    if is_test_file(path) {
        0
    } else if path.rsplit('/').next().is_some_and(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "index.ts" | "index.tsx" | "index.js" | "main.rs" | "lib.rs" | "main.py"
        )
    }) {
        1
    } else {
        2
    }
}

fn code_metrics(source: &str) -> (usize, usize) {
    source.lines().filter(|line| !line.trim().is_empty()).fold(
        (0, 0),
        |(lines, declarations), line| {
            let line = line.trim_start();
            let is_declaration = [
                "async fn ",
                "class ",
                "def ",
                "enum ",
                "export const ",
                "export function ",
                "fn ",
                "function ",
                "interface ",
                "pub fn ",
                "struct ",
                "type ",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix));
            (lines + 1, declarations + usize::from(is_declaration))
        },
    )
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

    #[test]
    fn source_inspection_filters_generated_and_dependency_files() {
        assert!(is_source_file("src/main.rs"));
        assert!(!is_source_file("target/debug/build.rs"));
        assert!(!is_source_file("node_modules/pkg/index.js"));
        assert!(is_test_file("src/agent_test.rs"));
        assert!(is_test_file("src/agent.spec.ts"));
    }

    #[test]
    fn code_metrics_count_nonblank_lines_and_declarations() {
        let (lines, declarations) =
            code_metrics("pub fn analyze() {}\n\nclass Agent:\n    def run(self):\n        pass\n");
        assert_eq!(lines, 4);
        assert_eq!(declarations, 3);
    }
}

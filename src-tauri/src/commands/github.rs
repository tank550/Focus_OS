use crate::infrastructure::github::{GithubClient, GithubRepository, GithubSourceSummary};
use crate::infrastructure::secrets;

#[tauri::command]
pub fn github_token_status() -> Result<bool, String> {
    secrets::has_github_token().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn github_token_store(token: String) -> Result<Vec<GithubRepository>, String> {
    let client = GithubClient::new(&token).map_err(|error| error.to_string())?;
    let repositories = client
        .list_owned_repositories()
        .await
        .map_err(|error| error.to_string())?;
    secrets::store_github_token(&token).map_err(|error| error.to_string())?;
    Ok(repositories)
}

#[tauri::command]
pub fn github_token_remove() -> Result<(), String> {
    secrets::remove_github_token().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn github_repositories() -> Result<Vec<GithubRepository>, String> {
    let token = secrets::load_github_token().map_err(|error| error.to_string())?;
    let client = GithubClient::new(&token).map_err(|error| error.to_string())?;
    client
        .list_owned_repositories()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn github_inspect_repository(full_name: String) -> Result<GithubSourceSummary, String> {
    let token = secrets::load_github_token().map_err(|error| error.to_string())?;
    let client = GithubClient::new(&token).map_err(|error| error.to_string())?;
    client
        .inspect_owned_repository(&full_name)
        .await
        .map_err(|error| error.to_string())
}

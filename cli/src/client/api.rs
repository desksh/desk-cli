//! Desk API client for backend communication.
//!
//! This module provides the main HTTP client for interacting with the Desk API.
//! It handles:
//! - Authentication header injection via middleware
//! - Automatic token refresh for expired tokens
//! - OAuth token exchange for API credentials

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use tokio::sync::RwLock;
use url::Url;

use crate::auth::{ApiCredentials, AuthProvider, CredentialStore, TokenSet};
use crate::client::middleware::{AuthMiddleware, TokenRefreshMiddleware};
use crate::config::ApiConfig;
use crate::error::{DeskError, Result};

/// Response from the token exchange endpoint.
#[derive(serde::Deserialize)]
struct TokenExchangeResponse {
    api_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    user_id: String,
}

/// Workspace state sent to/from the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceState {
    /// Git branch name.
    pub branch: String,
    /// Git commit SHA.
    pub commit_sha: String,
    /// Stash name if any.
    pub stash_name: Option<String>,
    /// Repository path (local to this machine).
    pub repo_path: String,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: WorkspaceStateMetadata,
}

/// Metadata within workspace state.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceStateMetadata {
    /// Number of uncommitted files.
    pub uncommitted_files: Option<u32>,
    /// Whether the working directory was dirty.
    pub was_dirty: Option<bool>,
}

/// Remote workspace from the API.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RemoteWorkspace {
    /// Remote workspace ID.
    pub id: String,
    /// Workspace name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Serialized workspace state.
    pub state: WorkspaceState,
    /// Version number for conflict detection.
    pub version: i32,
    /// Last sync timestamp.
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Main API client for communicating with the Desk backend.
///
/// Wraps an HTTP client with authentication and token refresh middleware.
/// Credentials are stored in memory and synchronized with the OS keyring.
pub struct DeskApiClient {
    client: ClientWithMiddleware,
    base_url: Url,
    credentials: Arc<RwLock<Option<ApiCredentials>>>,
}

impl DeskApiClient {
    /// Creates a new API client with the given configuration.
    ///
    /// Initializes the HTTP client with authentication and token refresh middleware.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built (e.g., TLS initialization fails).
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let inner_client = Client::builder()
            .user_agent(format!("desk-cli/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        let credentials: Arc<RwLock<Option<ApiCredentials>>> = Arc::new(RwLock::new(None));

        let client = ClientBuilder::new(inner_client.clone())
            .with(AuthMiddleware::new(Arc::clone(&credentials)))
            .with(TokenRefreshMiddleware::new(
                Arc::clone(&credentials),
                config.base_url.clone(),
                inner_client,
            ))
            .build();

        Ok(Self {
            client,
            base_url: config.base_url.clone(),
            credentials,
        })
    }

    /// Loads credentials from the OS keyring into memory.
    ///
    /// Should be called before making authenticated API requests.
    ///
    /// # Returns
    ///
    /// Returns `true` if credentials were found and loaded, `false` if none exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the keyring is inaccessible or credentials are corrupted.
    pub async fn load_credentials(&self) -> Result<bool> {
        let store = CredentialStore::new()?;
        if let Some(creds) = store.load()? {
            *self.credentials.write().await = Some(creds);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Sets credentials directly in memory (after successful authentication).
    ///
    /// Does not persist to the keyring; use [`CredentialStore::save`] for that.
    #[allow(dead_code)] // Kept for future API commands
    pub async fn set_credentials(&self, creds: ApiCredentials) {
        *self.credentials.write().await = Some(creds);
    }

    /// Clears credentials from memory.
    ///
    /// Does not delete from the keyring; use [`CredentialStore::delete`] for that.
    #[allow(dead_code)] // Kept for future logout enhancements
    pub async fn clear_credentials(&self) {
        *self.credentials.write().await = None;
    }

    /// Checks if the client has credentials loaded in memory.
    #[allow(dead_code)] // Kept for future API commands
    pub async fn is_authenticated(&self) -> bool {
        self.credentials.read().await.is_some()
    }

    /// Returns a clone of the current credentials, if any.
    pub async fn get_credentials(&self) -> Option<ApiCredentials> {
        self.credentials.read().await.clone()
    }

    /// Exchanges OAuth provider tokens for Desk API credentials.
    ///
    /// Called after successful OAuth authentication to obtain API access.
    ///
    /// # Arguments
    ///
    /// * `provider` - The OAuth provider used for authentication
    /// * `provider_tokens` - Tokens received from the OAuth provider
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API is unreachable ([`DeskError::ApiUnavailable`])
    /// - Authentication fails ([`DeskError::Unauthorized`])
    /// - The response cannot be parsed ([`DeskError::Serialization`])
    pub async fn exchange_token(
        &self,
        provider: AuthProvider,
        provider_tokens: &TokenSet,
    ) -> Result<ApiCredentials> {
        let url = self
            .base_url
            .join("/v1/auth/token")
            .map_err(|e| DeskError::Config(format!("Invalid token URL: {e}")))?;

        let body = serde_json::json!({
            "provider": provider.to_string(),
            "access_token": provider_tokens.access_token,
            "refresh_token": provider_tokens.refresh_token,
        });

        let response = self
            .client
            .post(url)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| format!("HTTP {status_code}"));

            return Err(match status_code {
                401 => DeskError::Unauthorized,
                503 => DeskError::ApiUnavailable,
                _ => DeskError::ApiError {
                    status: status_code,
                    message,
                },
            });
        }

        let data: TokenExchangeResponse = response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid token response: {e}")))?;

        Ok(ApiCredentials {
            provider,
            provider_tokens: provider_tokens.clone(),
            api_token: data.api_token,
            api_token_expires_at: data.expires_at,
            user_id: data.user_id,
        })
    }

    /// Returns the base URL for API requests.
    #[allow(dead_code)] // Kept for future API commands
    #[must_use]
    pub const fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns a reference to the underlying HTTP client.
    ///
    /// Use this for making custom API requests not covered by other methods.
    #[allow(dead_code)] // Kept for future API commands
    #[must_use]
    pub const fn client(&self) -> &ClientWithMiddleware {
        &self.client
    }

    // ========== Workspace API Methods ==========

    /// Lists all workspaces for the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The user is not authenticated ([`DeskError::Unauthorized`])
    /// - Pro subscription is required ([`DeskError::SubscriptionRequired`])
    /// - The API is unreachable ([`DeskError::ApiUnavailable`])
    pub async fn list_workspaces(&self) -> Result<Vec<RemoteWorkspace>> {
        let url = self
            .base_url
            .join("/v1/workspaces")
            .map_err(|e| DeskError::Config(format!("Invalid workspaces URL: {e}")))?;

        let response = self.client.get(url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status.as_u16(), response).await);
        }

        response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid workspaces response: {e}")))
    }

    /// Gets a workspace by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace is not found or access is denied.
    #[allow(dead_code)] // Will be used for single workspace operations
    pub async fn get_workspace(&self, id: &str) -> Result<RemoteWorkspace> {
        let url = self
            .base_url
            .join(&format!("/v1/workspaces/{id}"))
            .map_err(|e| DeskError::Config(format!("Invalid workspace URL: {e}")))?;

        let response = self.client.get(url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status.as_u16(), response).await);
        }

        response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid workspace response: {e}")))
    }

    /// Creates or updates a workspace by name (upsert).
    ///
    /// # Arguments
    ///
    /// * `name` - Workspace name
    /// * `description` - Optional description
    /// * `state` - Workspace state to save
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
        state: &WorkspaceState,
    ) -> Result<RemoteWorkspace> {
        let url = self
            .base_url
            .join("/v1/workspaces")
            .map_err(|e| DeskError::Config(format!("Invalid workspaces URL: {e}")))?;

        let body = serde_json::json!({
            "name": name,
            "description": description,
            "state": state,
        });

        let response = self
            .client
            .post(url)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status.as_u16(), response).await);
        }

        response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid workspace response: {e}")))
    }

    /// Updates a workspace with version checking.
    ///
    /// # Arguments
    ///
    /// * `id` - Remote workspace ID
    /// * `name` - Optional new name
    /// * `description` - Optional new description
    /// * `state` - Optional new state
    /// * `version` - Expected version for optimistic locking
    ///
    /// # Errors
    ///
    /// Returns [`DeskError::SyncConflict`] if the remote version doesn't match.
    pub async fn update_workspace(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        state: Option<&WorkspaceState>,
        version: i32,
    ) -> Result<RemoteWorkspace> {
        let url = self
            .base_url
            .join(&format!("/v1/workspaces/{id}"))
            .map_err(|e| DeskError::Config(format!("Invalid workspace URL: {e}")))?;

        let body = serde_json::json!({
            "name": name,
            "description": description,
            "state": state,
            "version": version,
        });

        let response = self
            .client
            .put(url)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status.as_u16(), response).await);
        }

        response
            .json()
            .await
            .map_err(|e| DeskError::Serialization(format!("Invalid workspace response: {e}")))
    }

    /// Deletes a workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace is not found or access is denied.
    #[allow(dead_code)] // Will be used for desk sync delete command
    pub async fn delete_workspace(&self, id: &str) -> Result<()> {
        let url = self
            .base_url
            .join(&format!("/v1/workspaces/{id}"))
            .map_err(|e| DeskError::Config(format!("Invalid workspace URL: {e}")))?;

        let response = self.client.delete(url).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_response(status.as_u16(), response).await);
        }

        Ok(())
    }

    /// Handles error responses from the API.
    async fn handle_error_response(
        &self,
        status_code: u16,
        response: reqwest::Response,
    ) -> DeskError {
        // Try to parse error response body
        #[derive(serde::Deserialize)]
        struct ErrorBody {
            error: Option<String>,
            message: Option<String>,
        }

        let error_body: Option<ErrorBody> = response.json().await.ok();
        let error_code = error_body.as_ref().and_then(|e| e.error.clone());
        let message = error_body
            .and_then(|e| e.message)
            .unwrap_or_else(|| format!("HTTP {status_code}"));

        match status_code {
            401 => DeskError::Unauthorized,
            403 => {
                // Check if it's a subscription error
                if error_code.as_deref() == Some("insufficient_tier") {
                    DeskError::SubscriptionRequired
                } else {
                    DeskError::ApiError {
                        status: status_code,
                        message,
                    }
                }
            },
            409 => {
                // Version conflict
                DeskError::ApiError {
                    status: status_code,
                    message,
                }
            },
            503 => DeskError::ApiUnavailable,
            _ => DeskError::ApiError {
                status: status_code,
                message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_serializes() {
        let state = WorkspaceState {
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            stash_name: Some("desk: workspace test".to_string()),
            repo_path: "/home/user/project".to_string(),
            metadata: WorkspaceStateMetadata {
                uncommitted_files: Some(5),
                was_dirty: Some(true),
            },
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: WorkspaceState = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.commit_sha, "abc123");
        assert_eq!(parsed.stash_name, Some("desk: workspace test".to_string()));
        assert_eq!(parsed.repo_path, "/home/user/project");
        assert_eq!(parsed.metadata.uncommitted_files, Some(5));
        assert_eq!(parsed.metadata.was_dirty, Some(true));
    }

    #[test]
    fn workspace_state_metadata_defaults() {
        let metadata = WorkspaceStateMetadata::default();

        assert!(metadata.uncommitted_files.is_none());
        assert!(metadata.was_dirty.is_none());
    }

    #[test]
    fn remote_workspace_deserializes() {
        let json = r#"{
            "id": "uuid-123",
            "name": "my-workspace",
            "description": "Test workspace",
            "state": {
                "branch": "feature/test",
                "commit_sha": "def456",
                "stash_name": null,
                "repo_path": "/repo",
                "metadata": {}
            },
            "version": 3,
            "last_synced_at": "2024-01-15T10:30:00Z",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-15T10:30:00Z"
        }"#;

        let ws: RemoteWorkspace = serde_json::from_str(json).unwrap();

        assert_eq!(ws.id, "uuid-123");
        assert_eq!(ws.name, "my-workspace");
        assert_eq!(ws.description, Some("Test workspace".to_string()));
        assert_eq!(ws.state.branch, "feature/test");
        assert_eq!(ws.version, 3);
        assert!(ws.last_synced_at.is_some());
    }

    #[test]
    fn remote_workspace_without_optional_fields() {
        let json = r#"{
            "id": "uuid-456",
            "name": "minimal-ws",
            "description": null,
            "state": {
                "branch": "main",
                "commit_sha": "abc",
                "stash_name": null,
                "repo_path": "/repo",
                "metadata": {}
            },
            "version": 1,
            "last_synced_at": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let ws: RemoteWorkspace = serde_json::from_str(json).unwrap();

        assert_eq!(ws.id, "uuid-456");
        assert!(ws.description.is_none());
        assert!(ws.last_synced_at.is_none());
        assert_eq!(ws.version, 1);
    }

    #[test]
    fn workspace_state_without_stash() {
        let state = WorkspaceState {
            branch: "develop".to_string(),
            commit_sha: "deadbeef".to_string(),
            stash_name: None,
            repo_path: "/path/to/repo".to_string(),
            metadata: WorkspaceStateMetadata::default(),
        };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: WorkspaceState = serde_json::from_str(&json).unwrap();

        assert!(parsed.stash_name.is_none());
        assert_eq!(parsed.branch, "develop");
    }

    #[test]
    fn workspace_state_metadata_with_values() {
        let metadata = WorkspaceStateMetadata {
            uncommitted_files: Some(10),
            was_dirty: Some(false),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let parsed: WorkspaceStateMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.uncommitted_files, Some(10));
        assert_eq!(parsed.was_dirty, Some(false));
    }

    #[test]
    fn remote_workspace_with_full_metadata() {
        let json = r#"{
            "id": "full-uuid",
            "name": "full-workspace",
            "description": "A fully populated workspace",
            "state": {
                "branch": "feature/full",
                "commit_sha": "1234567890abcdef",
                "stash_name": "desk: feature/full snapshot",
                "repo_path": "/home/dev/project",
                "metadata": {
                    "uncommitted_files": 7,
                    "was_dirty": true
                }
            },
            "version": 42,
            "last_synced_at": "2024-06-15T14:30:00Z",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-06-15T14:30:00Z"
        }"#;

        let ws: RemoteWorkspace = serde_json::from_str(json).unwrap();

        assert_eq!(ws.name, "full-workspace");
        assert_eq!(
            ws.state.stash_name,
            Some("desk: feature/full snapshot".to_string())
        );
        assert_eq!(ws.state.metadata.uncommitted_files, Some(7));
        assert_eq!(ws.state.metadata.was_dirty, Some(true));
        assert_eq!(ws.version, 42);
    }

    #[test]
    fn workspace_state_preserves_repo_path() {
        let paths = vec![
            "/home/user/projects/app",
            "/Users/dev/code/project",
            "C:\\Users\\Dev\\Projects",
            "/opt/workspace",
        ];

        for path in paths {
            let state = WorkspaceState {
                branch: "main".to_string(),
                commit_sha: "abc".to_string(),
                stash_name: None,
                repo_path: path.to_string(),
                metadata: WorkspaceStateMetadata::default(),
            };

            let json = serde_json::to_string(&state).unwrap();
            let parsed: WorkspaceState = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.repo_path, path);
        }
    }

    #[test]
    fn api_client_can_be_created() {
        let config = ApiConfig::default();
        let client = DeskApiClient::new(&config);

        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn api_client_starts_unauthenticated() {
        let config = ApiConfig::default();
        let client = DeskApiClient::new(&config).unwrap();

        assert!(!client.is_authenticated().await);
        assert!(client.get_credentials().await.is_none());
    }
}

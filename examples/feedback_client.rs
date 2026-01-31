// -----------------------------------------------------------------------------
// 🌮 Feedback API Client - Helping Smart Tree Survive the Franchise Wars!
// -----------------------------------------------------------------------------
// This module handles communication with f.8t.is for feedback submission and
// update checking. All feedback helps make Smart Tree better!
//
// Endpoints:
// - POST https://f.8t.is/api/feedback - Submit feedback and feature requests
// - GET  https://f.8t.is/mcp/check - Get latest version info with platform/arch (preferred)
// - GET  https://f.8t.is/api/smart-tree/latest - Get latest version info (legacy fallback)
// -----------------------------------------------------------------------------

use anyhow::Result;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const FEEDBACK_API_BASE: &str = "https://f.8t.is";
const USER_AGENT: &str = concat!("smart-tree/", env!("CARGO_PKG_VERSION"));

/// Feedback submission request structure
#[derive(Debug, Serialize)]
pub struct FeedbackRequest {
    pub category: String,
    pub title: String,
    pub description: String,
    pub impact_score: u8,
    pub frequency_score: u8,
    pub affected_command: Option<String>,
    pub mcp_tool: Option<String>,
    pub proposed_fix: Option<String>,
    pub proposed_solution: Option<String>,
    pub fix_complexity: Option<String>,
    pub auto_fixable: Option<bool>,
    pub tags: Vec<String>,
    pub examples: Vec<FeedbackExample>,
    pub smart_tree_version: String,
    pub anonymous: bool,
    pub github_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeedbackExample {
    pub description: String,
    pub code: String,
    pub expected_output: Option<String>,
}

/// Tool request structure
#[derive(Debug, Serialize)]
pub struct ToolRequest {
    pub tool_name: String,
    pub description: String,
    pub use_case: String,
    pub expected_output: String,
    pub productivity_impact: String,
    pub proposed_parameters: Option<Value>,
    pub smart_tree_version: String,
    pub anonymous: bool,
    pub github_url: Option<String>,
}

/// Response from feedback API
#[derive(Debug, Deserialize)]
pub struct FeedbackResponse {
    pub feedback_id: String,
    pub message: String,
    pub status: String,
}

/// Latest version info from legacy endpoint
#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub release_date: String,
    pub download_url: String,
    pub release_notes_url: String,
    pub features: Vec<String>,
    pub ai_benefits: Vec<String>,
}

/// Response from MCP check endpoint
#[derive(Debug, Deserialize)]
pub struct McpCheckResponse {
    pub latest_version: String,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub new_features: Option<Vec<String>>,
    pub message: Option<String>,
}

/// API client for f.8t.is
pub struct FeedbackClient {
    client: Client,
}

impl FeedbackClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self { client })
    }

    /// Submit feedback to f.8t.is
    pub async fn submit_feedback(&self, feedback: FeedbackRequest) -> Result<FeedbackResponse> {
        let url = format!("{}/api/feedback", FEEDBACK_API_BASE);

        let response = self.client.post(&url).json(&feedback).send().await?;

        match response.status() {
            StatusCode::OK => {
                let data = response.json::<FeedbackResponse>().await?;
                Ok(data)
            }
            StatusCode::TOO_MANY_REQUESTS => Err(anyhow::anyhow!(
                "Rate limit exceeded. Please try again later."
            )),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(anyhow::anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }

    /// Submit tool request to f.8t.is
    pub async fn submit_tool_request(&self, request: ToolRequest) -> Result<FeedbackResponse> {
        let url = format!("{}/api/tool-request", FEEDBACK_API_BASE);

        let response = self.client.post(&url).json(&request).send().await?;

        match response.status() {
            StatusCode::OK => {
                let data = response.json::<FeedbackResponse>().await?;
                Ok(data)
            }
            StatusCode::TOO_MANY_REQUESTS => Err(anyhow::anyhow!(
                "Rate limit exceeded. Please try again later."
            )),
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(anyhow::anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }

    /// Check for latest version using the new MCP endpoint with fallback to legacy
    ///
    /// This function attempts to use the new `/mcp/check` endpoint which provides
    /// platform and architecture analytics. If that fails for any reason, it falls
    /// back to the legacy `/api/smart-tree/latest` endpoint.
    ///
    /// Note: When using the MCP endpoint, some fields in the returned `VersionInfo`
    /// may contain placeholder values ("N/A" or empty vectors) as the MCP endpoint
    /// provides a different data structure.
    pub async fn check_for_updates(&self) -> Result<VersionInfo> {
        let current_version = env!("CARGO_PKG_VERSION");

        // Try the new MCP endpoint first (with platform and architecture detection)
        let platform = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let mcp_url = format!(
            "{}/mcp/check?version={}&platform={}&arch={}",
            FEEDBACK_API_BASE, current_version, platform, arch
        );

        // Attempt to use the new MCP endpoint
        match self.client.get(&mcp_url).send().await {
            Ok(response) if response.status() == StatusCode::OK => {
                if let Ok(mcp_data) = response.json::<McpCheckResponse>().await {
                    // Convert MCP response to VersionInfo format
                    // Note: Some fields use placeholders as MCP endpoint has different schema
                    return Ok(VersionInfo {
                        version: mcp_data.latest_version,
                        release_date: "N/A".to_string(), // MCP endpoint doesn't provide this
                        download_url: mcp_data.download_url.unwrap_or_default(),
                        release_notes_url: mcp_data.release_notes.unwrap_or_default(),
                        features: mcp_data.new_features.unwrap_or_default(),
                        ai_benefits: vec![], // MCP endpoint doesn't provide this
                    });
                }
            }
            Err(e) => {
                // Log MCP endpoint failure for debugging (only in debug mode to avoid production noise)
                #[cfg(debug_assertions)]
                eprintln!(
                    "Debug: MCP endpoint failed ({}), falling back to legacy endpoint",
                    e
                );
            }
            _ => {
                // Non-200 status from MCP endpoint, fall through to legacy
                #[cfg(debug_assertions)]
                eprintln!(
                    "Debug: MCP endpoint returned non-OK status, falling back to legacy endpoint"
                );
            }
        }

        // Fall back to the legacy /api/smart-tree/latest endpoint
        let legacy_url = format!("{}/api/smart-tree/latest", FEEDBACK_API_BASE);
        let response = self.client.get(&legacy_url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let data = response.json::<VersionInfo>().await?;
                Ok(data)
            }
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(anyhow::anyhow!("API error ({}): {}", status, error_text))
            }
        }
    }
}

impl Default for FeedbackClient {
    fn default() -> Self {
        Self::new().expect("Failed to create feedback client")
    }
}

/// Example main function to demonstrate the feedback client
#[tokio::main]
async fn main() -> Result<()> {
    println!("Feedback Client Example");
    println!("{}", "=".repeat(40));

    // Create a feedback client
    let client = FeedbackClient::new()?;
    println!("Feedback client created successfully!");

    // Check for updates
    println!("\nChecking for Smart Tree updates...");
    match client.check_for_updates().await {
        Ok(info) => {
            println!("Latest version: {}", info.version);
            println!("Release date: {}", info.release_date);
            println!("Features: {:?}", info.features);
        }
        Err(e) => println!("Failed to check for updates: {}", e),
    }

    println!("\nExample complete!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_client_creation() {
        let client = FeedbackClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_mcp_check_response_deserialization() {
        let json = r#"{
            "latest_version": "1.0.0",
            "update_available": false,
            "download_url": "https://example.com/download",
            "release_notes": "Some notes",
            "new_features": ["feature1", "feature2"],
            "message": "Up to date"
        }"#;

        let response: Result<McpCheckResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.latest_version, "1.0.0");
        assert!(!response.update_available);
    }

    #[test]
    fn test_version_info_deserialization() {
        let json = r#"{
            "version": "1.0.0",
            "release_date": "2024-01-01",
            "download_url": "https://example.com/download",
            "release_notes_url": "https://example.com/notes",
            "features": ["feature1"],
            "ai_benefits": ["benefit1"]
        }"#;

        let info: Result<VersionInfo, _> = serde_json::from_str(json);
        assert!(info.is_ok());

        let info = info.unwrap();
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.release_date, "2024-01-01");
    }
}

// 🤖 MCP (Model Context Protocol) Handler - Smart Tree Integration! 🤖
// Logs and responds to MCP tool requests from Smart Tree clients
// Created with love by Aye & Hue! ✨

use crate::api::AppState;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{info, debug};
use chrono::Utc;

/// 📊 MCP Check Request - Version and platform info from Smart Tree clients
#[derive(Debug, Deserialize)]
pub struct McpCheckQuery {
    pub version: Option<String>,
    pub platform: Option<String>,
    pub arch: Option<String>,
}

/// 📊 MCP Check Response
#[derive(Debug, Serialize)]
pub struct McpCheckResponse {
    pub latest_version: String,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub new_features: Option<Vec<String>>,
    pub message: Option<String>,
}

/// 📊 MCP Analytics Entry (for database logging)
#[derive(Debug, Serialize)]
pub struct McpAnalytics {
    pub client_version: String,
    pub platform: String,
    pub arch: String,
    pub timestamp: String,
}

/// 🔍 GET /mcp/check - Handle version check requests from Smart Tree
///
/// This endpoint is called by Smart Tree MCP clients to check for updates.
/// It logs platform/version info for analytics and returns update info.
pub async fn mcp_check(
    State(app_state): State<AppState>,
    Query(query): Query<McpCheckQuery>,
) -> impl IntoResponse {
    let version = query.version.unwrap_or_else(|| "unknown".to_string());
    let platform = query.platform.unwrap_or_else(|| "unknown".to_string());
    let arch = query.arch.unwrap_or_else(|| "unknown".to_string());

    info!(
        "📊 MCP check received - version: {}, platform: {}, arch: {}",
        version, platform, arch
    );

    // Log to database for analytics
    if let Err(e) = log_mcp_analytics(&app_state, &version, &platform, &arch).await {
        debug!("Failed to log MCP analytics: {}", e);
    }

    // TODO: Get actual latest version from releases table or config
    // For now, just echo back that they're up to date
    let latest_version = get_latest_smart_tree_version(&app_state).await
        .unwrap_or_else(|| version.clone());

    let update_available = is_newer_version(&latest_version, &version);

    // Get release notes and features if available
    let (release_notes, new_features) = if update_available {
        let notes = get_release_notes(&app_state).await;
        let features = get_new_features(&app_state).await;
        (notes, features)
    } else {
        (None, None)
    };

    let response = McpCheckResponse {
        latest_version: latest_version.clone(),
        update_available,
        download_url: if update_available {
            Some(format!("https://github.com/8b-is/smart-tree/releases/tag/v{}", latest_version))
        } else {
            None
        },
        release_notes,
        new_features,
        message: Some("Thanks for using Smart Tree! 🌲".to_string()),
    };

    Json(response)
}

/// 📊 MCP Stats Response
#[derive(Debug, Serialize)]
pub struct McpStatsResponse {
    pub total_checks: i64,
    pub unique_platforms: Vec<PlatformStats>,
    pub version_distribution: Vec<VersionStats>,
    pub recent_checks: Vec<RecentCheck>,
}

#[derive(Debug, Serialize)]
pub struct PlatformStats {
    pub platform: String,
    pub arch: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct VersionStats {
    pub version: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct RecentCheck {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub checked_at: String,
}

/// 📊 GET /mcp/stats - Get MCP usage statistics (admin only)
pub async fn mcp_stats(State(app_state): State<AppState>) -> impl IntoResponse {
    info!("📊 MCP stats requested");

    let stats = get_mcp_stats(&app_state).await.unwrap_or_else(|_| McpStatsResponse {
        total_checks: 0,
        unique_platforms: vec![],
        version_distribution: vec![],
        recent_checks: vec![],
    });

    Json(stats)
}

/// 🔧 POST /mcp/version - Set the latest Smart Tree version (admin only)
#[derive(Debug, Deserialize)]
pub struct SetVersionRequest {
    pub version: String,
    pub release_notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SetVersionResponse {
    pub success: bool,
    pub version: String,
    pub message: String,
}

pub async fn mcp_set_version(
    State(app_state): State<AppState>,
    Json(request): Json<SetVersionRequest>,
) -> impl IntoResponse {
    info!("🔧 Setting Smart Tree version to: {}", request.version);

    match set_latest_version(&app_state, &request.version, request.release_notes.as_deref()).await {
        Ok(_) => Json(SetVersionResponse {
            success: true,
            version: request.version,
            message: "Version updated successfully".to_string(),
        }),
        Err(e) => Json(SetVersionResponse {
            success: false,
            version: request.version,
            message: format!("Failed to update version: {}", e),
        }),
    }
}

// Helper functions

/// Log MCP analytics to database
async fn log_mcp_analytics(
    app_state: &AppState,
    version: &str,
    platform: &str,
    arch: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO mcp_analytics (client_version, platform, arch, checked_at)
        VALUES ($1, $2, $3, NOW())
        "#
    )
    .bind(version)
    .bind(platform)
    .bind(arch)
    .execute(&app_state.db_pool)
    .await?;

    Ok(())
}

/// Get the latest Smart Tree version from settings
async fn get_latest_smart_tree_version(app_state: &AppState) -> Option<String> {
    let result = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_latest_version'"
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .ok()?;

    result
}

/// Get release notes from settings
async fn get_release_notes(app_state: &AppState) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_release_notes'"
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .ok()
    .flatten()
}

/// Get new features list from settings (stored as JSON array)
async fn get_new_features(app_state: &AppState) -> Option<Vec<String>> {
    let json_str = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_new_features'"
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .ok()
    .flatten()?;

    serde_json::from_str(&json_str).ok()
}

/// Set the latest Smart Tree version
async fn set_latest_version(
    app_state: &AppState,
    version: &str,
    release_notes: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO settings (key, value, updated_at)
        VALUES ('smart_tree_latest_version', $1, NOW())
        ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()
        "#
    )
    .bind(version)
    .execute(&app_state.db_pool)
    .await?;

    if let Some(notes) = release_notes {
        sqlx::query(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES ('smart_tree_release_notes', $1, NOW())
            ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()
            "#
        )
        .bind(notes)
        .execute(&app_state.db_pool)
        .await?;
    }

    Ok(())
}

/// Get MCP statistics
async fn get_mcp_stats(app_state: &AppState) -> anyhow::Result<McpStatsResponse> {
    // Total checks
    let total_checks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_analytics")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    // Platform distribution
    let platform_rows = sqlx::query(
        r#"
        SELECT platform, arch, COUNT(*) as count
        FROM mcp_analytics
        GROUP BY platform, arch
        ORDER BY count DESC
        LIMIT 20
        "#
    )
    .fetch_all(&app_state.db_pool)
    .await
    .unwrap_or_default();

    let unique_platforms: Vec<PlatformStats> = platform_rows
        .iter()
        .map(|row| PlatformStats {
            platform: row.get("platform"),
            arch: row.get("arch"),
            count: row.get("count"),
        })
        .collect();

    // Version distribution
    let version_rows = sqlx::query(
        r#"
        SELECT client_version as version, COUNT(*) as count
        FROM mcp_analytics
        GROUP BY client_version
        ORDER BY count DESC
        LIMIT 20
        "#
    )
    .fetch_all(&app_state.db_pool)
    .await
    .unwrap_or_default();

    let version_distribution: Vec<VersionStats> = version_rows
        .iter()
        .map(|row| VersionStats {
            version: row.get("version"),
            count: row.get("count"),
        })
        .collect();

    // Recent checks
    let recent_rows = sqlx::query(
        r#"
        SELECT client_version, platform, arch, checked_at
        FROM mcp_analytics
        ORDER BY checked_at DESC
        LIMIT 50
        "#
    )
    .fetch_all(&app_state.db_pool)
    .await
    .unwrap_or_default();

    let recent_checks: Vec<RecentCheck> = recent_rows
        .iter()
        .map(|row| RecentCheck {
            version: row.get("client_version"),
            platform: row.get("platform"),
            arch: row.get("arch"),
            checked_at: row.get::<chrono::DateTime<chrono::Utc>, _>("checked_at")
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        })
        .collect();

    Ok(McpStatsResponse {
        total_checks,
        unique_platforms,
        version_distribution,
        recent_checks,
    })
}

/// Compare semantic versions to check if there's an update
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);

    for i in 0..latest_parts.len().max(current_parts.len()) {
        let l = latest_parts.get(i).unwrap_or(&0);
        let c = current_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.1.0", "1.0.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.1.0"));
        assert!(!is_newer_version("0.9.0", "1.0.0"));
        println!("✅ Version comparison tests passed!");
    }
}

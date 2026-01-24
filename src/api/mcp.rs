// 🤖 MCP (Model Context Protocol) Handler - Smart Tree Integration! 🤖
// Logs and responds to MCP tool requests from Smart Tree clients
// Created with love by Aye & Hue! ✨

use crate::api::AppState;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

/// 🌍 GeoIP Database (loaded once)
static GEOIP_DB: OnceLock<Option<maxminddb::Reader<Vec<u8>>>> = OnceLock::new();

/// 🌍 Database download state (to prevent concurrent downloads)
static DOWNLOAD_INIT: OnceCell<()> = OnceCell::const_new();

/// 🌍 GeoIP lookup result
#[derive(Debug, Default, Clone)]
pub struct GeoLocation {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// 🌍 Paths to check for GeoIP database
const GEOIP_PATHS: &[&str] = &[
    "/usr/share/GeoIP/GeoLite2-City.mmdb",
    "/var/lib/GeoIP/GeoLite2-City.mmdb",
    "./GeoLite2-City.mmdb",
    "/app/GeoLite2-City.mmdb",
    "/data/GeoLite2-City.mmdb",
];

/// 🌍 Default download path for GeoIP database
const GEOIP_DOWNLOAD_PATH: &str = "./GeoLite2-City.mmdb";

/// 🌍 MaxMind download URL template
const MAXMIND_DOWNLOAD_URL: &str =
    "https://download.maxmind.com/geoip/databases/GeoLite2-City/download?suffix=tar.gz";

/// 🌍 Database refresh interval (default: 24 hours, MaxMind updates weekly)
const DEFAULT_REFRESH_HOURS: u64 = 24;

/// 🌍 Initialize GeoIP database (with optional auto-download)
fn get_geoip_reader() -> Option<&'static maxminddb::Reader<Vec<u8>>> {
    GEOIP_DB
        .get_or_init(|| {
            // First, try to load from existing paths
            for path in GEOIP_PATHS {
                if let Ok(reader) = maxminddb::Reader::open_readfile(path) {
                    info!("🌍 GeoIP database loaded from: {}", path);
                    return Some(reader);
                }
            }

            warn!("🌍 GeoIP database not found - location tracking disabled");
            warn!("🌍 Set MAXMIND_ACCOUNT_ID and MAXMIND_LICENSE_KEY to enable auto-download");
            None
        })
        .as_ref()
}

/// 🌍 Initialize GeoIP database with auto-download support
/// Call this during app startup to download the database if needed
pub async fn init_geoip_database() {
    // Only run once
    DOWNLOAD_INIT
        .get_or_init(|| async {
            let account_id = std::env::var("MAXMIND_ACCOUNT_ID").ok();
            let license_key = std::env::var("MAXMIND_LICENSE_KEY").ok();

            // Check if database already exists and is fresh
            let mut existing_path: Option<&str> = None;
            let mut needs_refresh = false;

            for path in GEOIP_PATHS {
                if Path::new(path).exists() {
                    existing_path = Some(path);
                    // Check if database is stale (older than refresh interval)
                    if let Ok(metadata) = std::fs::metadata(path) {
                        if let Ok(modified) = metadata.modified() {
                            let age = std::time::SystemTime::now()
                                .duration_since(modified)
                                .unwrap_or_default();
                            let refresh_hours = std::env::var("GEOIP_REFRESH_HOURS")
                                .ok()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(DEFAULT_REFRESH_HOURS);
                            let max_age = std::time::Duration::from_secs(refresh_hours * 3600);

                            if age > max_age {
                                info!(
                                    "🌍 GeoIP database is {} hours old, will refresh",
                                    age.as_secs() / 3600
                                );
                                needs_refresh = true;
                            }
                        }
                    }
                    break;
                }
            }

            if existing_path.is_some() && !needs_refresh {
                info!("🌍 GeoIP database found at: {}", existing_path.unwrap());
            }

            // Download if missing or stale (and credentials are available)
            if existing_path.is_none() || needs_refresh {
                if let (Some(account_id), Some(license_key)) = (&account_id, &license_key) {
                    if !account_id.is_empty() && !license_key.is_empty() {
                        let action = if existing_path.is_none() {
                            "download"
                        } else {
                            "refresh"
                        };
                        info!("🌍 Attempting to {} GeoIP database from MaxMind...", action);
                        if let Err(e) = download_geoip_database(account_id, license_key).await {
                            warn!("🌍 Failed to {} GeoIP database: {}", action, e);
                        }
                    }
                }
            }

            // Spawn background refresh task if credentials are available
            if let (Some(account_id), Some(license_key)) = (account_id, license_key) {
                if !account_id.is_empty() && !license_key.is_empty() {
                    spawn_refresh_task(account_id, license_key);
                }
            }
        })
        .await;
}

/// 🌍 Spawn background task to periodically refresh the database
fn spawn_refresh_task(account_id: String, license_key: String) {
    let refresh_hours = std::env::var("GEOIP_REFRESH_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_REFRESH_HOURS);

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(refresh_hours * 3600);
        info!(
            "🌍 GeoIP auto-refresh enabled: checking every {} hours",
            refresh_hours
        );

        loop {
            tokio::time::sleep(interval).await;
            info!("🌍 Running scheduled GeoIP database refresh...");
            if let Err(e) = download_geoip_database(&account_id, &license_key).await {
                warn!("🌍 Scheduled GeoIP refresh failed: {}", e);
            } else {
                info!("🌍 GeoIP database refreshed successfully");
            }
        }
    });
}

/// 🌍 Download GeoIP database from MaxMind
async fn download_geoip_database(account_id: &str, license_key: &str) -> anyhow::Result<()> {
    use std::io::Write;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    info!("🌍 Downloading GeoLite2-City database...");

    // Download the tar.gz file
    let response = client
        .get(MAXMIND_DOWNLOAD_URL)
        .basic_auth(account_id, Some(license_key))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "MaxMind API returned {}: {}",
            status,
            if body.len() > 200 {
                &body[..200]
            } else {
                &body
            }
        );
    }

    let bytes = response.bytes().await?;
    info!("🌍 Downloaded {} bytes, extracting...", bytes.len());

    // Extract the .mmdb file from the tar.gz archive
    let decoder = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.extension().map(|e| e == "mmdb").unwrap_or(false) {
            // Found the .mmdb file, extract it
            let mut contents = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut contents)?;

            // Write to download path
            let mut file = std::fs::File::create(GEOIP_DOWNLOAD_PATH)?;
            file.write_all(&contents)?;

            info!(
                "🌍 GeoIP database saved to: {} ({} bytes)",
                GEOIP_DOWNLOAD_PATH,
                contents.len()
            );
            return Ok(());
        }
    }

    anyhow::bail!("No .mmdb file found in downloaded archive")
}

/// 🌍 Look up geo location for IP
fn lookup_geo(ip: IpAddr) -> GeoLocation {
    let Some(reader) = get_geoip_reader() else {
        return GeoLocation::default();
    };

    // Skip private/local IPs
    match ip {
        IpAddr::V4(v4) if v4.is_private() || v4.is_loopback() => return GeoLocation::default(),
        IpAddr::V6(v6) if v6.is_loopback() => return GeoLocation::default(),
        _ => {}
    }

    match reader.lookup::<maxminddb::geoip2::City>(ip) {
        Ok(city_data) => GeoLocation {
            country: city_data.country.and_then(|c| c.iso_code).map(String::from),
            region: city_data
                .subdivisions
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.names.as_ref())
                .and_then(|n| n.get("en"))
                .map(|s| s.to_string()),
            city: city_data
                .city
                .and_then(|c| c.names)
                .and_then(|n| n.get("en").map(|s| s.to_string())),
            latitude: city_data.location.as_ref().and_then(|l| l.latitude),
            longitude: city_data.location.as_ref().and_then(|l| l.longitude),
        },
        Err(_) => GeoLocation::default(),
    }
}

/// 🔍 Extract client IP from request headers or connection
fn extract_client_ip(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
) -> Option<IpAddr> {
    // Check X-Forwarded-For first (for reverse proxies)
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            // Take the first IP (original client)
            if let Some(first_ip) = xff_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // Check X-Real-IP
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }

    // Fall back to connection info
    connect_info.map(|ci| ci.0.ip())
}

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
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Query(query): Query<McpCheckQuery>,
) -> impl IntoResponse {
    let version = query.version.unwrap_or_else(|| "unknown".to_string());
    let platform = query.platform.unwrap_or_else(|| "unknown".to_string());
    let arch = query.arch.unwrap_or_else(|| "unknown".to_string());

    // Extract client IP and do geo lookup
    let client_ip = extract_client_ip(&headers, connect_info.as_ref());
    let geo = client_ip.map(lookup_geo).unwrap_or_default();

    info!(
        "📊 MCP check received - version: {}, platform: {}, arch: {}, ip: {:?}, location: {:?}/{:?}",
        version, platform, arch, client_ip, geo.city, geo.country
    );

    // Log to database for analytics (with geo data)
    if let Err(e) = log_mcp_analytics(&app_state, &version, &platform, &arch, client_ip, &geo).await
    {
        debug!("Failed to log MCP analytics: {}", e);
    }

    // TODO: Get actual latest version from releases table or config
    // For now, just echo back that they're up to date
    let latest_version = get_latest_smart_tree_version(&app_state)
        .await
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
            Some(format!(
                "https://github.com/8b-is/smart-tree/releases/tag/v{}",
                latest_version
            ))
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

    let stats = get_mcp_stats(&app_state)
        .await
        .unwrap_or_else(|_| McpStatsResponse {
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

    match set_latest_version(
        &app_state,
        &request.version,
        request.release_notes.as_deref(),
    )
    .await
    {
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

/// Log MCP analytics to database (with geo data)
async fn log_mcp_analytics(
    app_state: &AppState,
    version: &str,
    platform: &str,
    arch: &str,
    ip: Option<IpAddr>,
    geo: &GeoLocation,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO mcp_analytics (
            client_version, platform, arch, checked_at,
            ip_address, country, region, city, latitude, longitude
        )
        VALUES ($1, $2, $3, NOW(), $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(version)
    .bind(platform)
    .bind(arch)
    .bind(ip.map(|ip| ip.to_string()))
    .bind(&geo.country)
    .bind(&geo.region)
    .bind(&geo.city)
    .bind(geo.latitude)
    .bind(geo.longitude)
    .execute(&app_state.db_pool)
    .await?;

    Ok(())
}

/// Get the latest Smart Tree version from settings
async fn get_latest_smart_tree_version(app_state: &AppState) -> Option<String> {
    let result = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_latest_version'",
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .ok()?;

    result
}

/// Get release notes from settings
async fn get_release_notes(app_state: &AppState) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_release_notes'",
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .ok()
    .flatten()
}

/// Get new features list from settings (stored as JSON array)
async fn get_new_features(app_state: &AppState) -> Option<Vec<String>> {
    let json_str = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'smart_tree_new_features'",
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
        "#,
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
            "#,
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
        "#,
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
        "#,
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
        "#,
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
            checked_at: row
                .get::<chrono::DateTime<chrono::Utc>, _>("checked_at")
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

// 🏃‍♂️ Database Migrations - Keeping Our Schema Up to Date! 🏃‍♂️
// Built with SQLx for safe, transactional schema updates! 🔒
// Created with love by Aye & Hue ✨

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use tracing::{info, warn};

/// 📋 Migration structure
#[derive(Debug, Clone)]
pub struct Migration {
    pub id: String,
    pub description: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
}

/// 📋 Create the migrations tracking table
pub async fn create_migrations_table(pool: &PgPool) -> Result<()> {
    info!("📋 Creating migrations tracking table...");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS migrations (
            id VARCHAR(255) PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            checksum VARCHAR(64) NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create migrations table")?;

    info!("✅ Migrations tracking table ready!");
    Ok(())
}

/// 🏃‍♂️ Run all pending migrations
pub async fn run_all_migrations(pool: &PgPool) -> Result<()> {
    info!("🚀 Starting migration process...");

    let migrations = get_all_migrations();
    let applied_migrations = get_applied_migrations(pool).await?;

    let mut applied_count = 0;

    for migration in migrations {
        if !applied_migrations.contains(&migration.id) {
            info!("📝 Applying migration: {} - {}", migration.id, migration.description);
            apply_migration(pool, &migration)
                .await
                .with_context(|| format!("Failed to apply migration {}", migration.id))?;
            applied_count += 1;
        }
    }

    if applied_count > 0 {
        info!("✅ Applied {} new migrations!", applied_count);
    } else {
        info!("✅ Database schema is up to date!");
    }

    Ok(())
}

/// 📝 Apply a single migration
async fn apply_migration(pool: &PgPool, migration: &Migration) -> Result<()> {
    let mut transaction = pool.begin().await.context("Failed to start transaction")?;

    // Execute each SQL statement separately
    for statement in split_sql_statements(&migration.up_sql) {
        // Strip leading comment lines from statement
        let cleaned: String = statement
            .lines()
            .skip_while(|line| line.trim().is_empty() || line.trim().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");

        let trimmed = cleaned.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed)
                .execute(&mut *transaction)
                .await
                .with_context(|| format!("SQL error: {}...", &trimmed[..trimmed.len().min(80)]))?;
        }
    }

    // Record migration
    let checksum = calculate_checksum(&migration.up_sql);
    sqlx::query("INSERT INTO migrations (id, description, checksum) VALUES ($1, $2, $3)")
        .bind(&migration.id)
        .bind(&migration.description)
        .bind(&checksum)
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await?;
    info!("✅ Migration {} applied!", migration.id);
    Ok(())
}

/// 🔍 Get applied migrations
async fn get_applied_migrations(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query("SELECT id FROM migrations ORDER BY applied_at")
        .fetch_all(pool)
        .await
        .context("Failed to fetch applied migrations")?;

    Ok(rows.into_iter().map(|row| row.get::<String, _>("id")).collect())
}

/// 🔢 Calculate checksum
fn calculate_checksum(sql: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 🔪 Split SQL into statements (handles $$ functions and parentheses)
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_dollar_quote = false;
    let mut paren_depth: i32 = 0;

    for ch in sql.chars() {
        current.push(ch);

        // Track $$ blocks for PL/pgSQL
        if current.ends_with("$$") {
            in_dollar_quote = !in_dollar_quote;
        }

        // Track parentheses (but not inside $$ blocks)
        if !in_dollar_quote {
            match ch {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                ';' if paren_depth == 0 => {
                    // End of statement
                    statements.push(current.clone());
                    current.clear();
                }
                _ => {}
            }
        }
    }

    if !current.trim().is_empty() {
        statements.push(current);
    }

    statements
}

/// 📚 All migrations - Fresh v1 schema
pub fn get_all_migrations() -> Vec<Migration> {
    vec![
        Migration {
            id: "v1_initial_schema".to_string(),
            description: "Complete initial schema for Feedbacker".to_string(),
            up_sql: r#"
-- Enum types
CREATE TYPE feedback_status AS ENUM ('pending', 'processing', 'generating_changes', 'creating_pull_request', 'completed', 'failed', 'paused');
CREATE TYPE user_role AS ENUM ('user', 'admin', 'service');
CREATE TYPE notification_type AS ENUM ('feedback_completed', 'feedback_failed', 'pull_request_created', 'system_update', 'warning');

-- Users
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    github_username VARCHAR(255) UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    role user_role NOT NULL DEFAULT 'user',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_github_username ON users(github_username);

-- User sessions
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at);

-- Projects
CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    repository VARCHAR(255) NOT NULL,
    description TEXT,
    default_llm_provider VARCHAR(50),
    system_message TEXT,
    config JSONB,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at TIMESTAMPTZ,
    UNIQUE(owner_id, repository)
);
CREATE INDEX idx_projects_owner_id ON projects(owner_id);
CREATE INDEX idx_projects_repository ON projects(repository);

-- Feedback
CREATE TABLE feedback (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    repository VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    status feedback_status NOT NULL DEFAULT 'pending',
    branch_name VARCHAR(255),
    pull_request_url TEXT,
    llm_provider VARCHAR(50),
    metadata JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_feedback_repository ON feedback(repository);
CREATE INDEX idx_feedback_status ON feedback(status);
CREATE INDEX idx_feedback_created_at ON feedback(created_at);

-- Rate limits
CREATE TABLE rate_limits (
    id VARCHAR(255) PRIMARY KEY,
    limit_type VARCHAR(50) NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,
    window_start TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_request TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Notifications
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type notification_type NOT NULL,
    title VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    related_id UUID,
    is_read BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    read_at TIMESTAMPTZ
);
CREATE INDEX idx_notifications_user_id ON notifications(user_id);

-- Webhooks
CREATE TABLE webhooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    processed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX idx_webhooks_project_id ON webhooks(project_id);

-- Background jobs
CREATE TABLE background_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    retries INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_background_jobs_status ON background_jobs(status);

-- Auto-update trigger
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON projects FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_feedback_updated_at BEFORE UPDATE ON feedback FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
            "#.to_string(),
            down_sql: Some("DROP SCHEMA public CASCADE; CREATE SCHEMA public;".to_string()),
        },
    ]
}

/// 🔙 Rollback (for development)
pub async fn rollback_migration(pool: &PgPool, migration_id: &str) -> Result<()> {
    warn!("⚠️ Rolling back migration: {}", migration_id);

    let migrations = get_all_migrations();
    let migration = migrations.iter().find(|m| m.id == migration_id).context("Migration not found")?;

    if let Some(down_sql) = &migration.down_sql {
        let mut tx = pool.begin().await?;

        for statement in split_sql_statements(down_sql) {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&mut *tx).await?;
            }
        }

        sqlx::query("DELETE FROM migrations WHERE id = $1")
            .bind(migration_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        info!("✅ Rolled back {}", migration_id);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_splitting() {
        let sql = "CREATE TABLE a (id INT);\nCREATE TABLE b (id INT);";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
    }

    #[test]
    fn test_sql_splitting_multiline_table() {
        let sql = r#"
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL
);
CREATE INDEX idx_users_email ON users(email);
"#;
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("CREATE TABLE"));
        assert!(statements[0].contains("email VARCHAR"));
        assert!(statements[1].contains("CREATE INDEX"));
    }

    #[test]
    fn test_actual_migration_sql() {
        let migrations = get_all_migrations();
        let migration = &migrations[0];
        let statements = split_sql_statements(&migration.up_sql);

        println!("\n=== SPLIT STATEMENTS ({} total) ===", statements.len());
        for (i, stmt) in statements.iter().enumerate() {
            let preview: String = stmt.chars().take(80).collect();
            println!("{}. {}", i + 1, preview.replace('\n', " "));
        }

        // First statement should be CREATE TYPE, not CREATE INDEX
        assert!(!statements[0].trim().starts_with("CREATE INDEX"),
            "First statement should not be CREATE INDEX!");
        assert!(statements.iter().any(|s| s.contains("CREATE TABLE users")),
            "Should have CREATE TABLE users statement");
    }
}

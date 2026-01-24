// 🔧 Admin Interface - System Management Dashboard! 🔧
// Created with love by Aye & Hue! ✨

use crate::api::AppState;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Response, Redirect},
    http::{StatusCode, HeaderMap, header},
    Json,
    Form,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{info, warn};

/// 🔐 Admin session cookie name
const ADMIN_SESSION_COOKIE: &str = "feedbacker_admin_session";

/// 🔐 Login form data
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// 🔐 Check if admin is authenticated via cookie
fn is_admin_authenticated(jar: &CookieJar, app_state: &AppState) -> bool {
    if app_state.config.auth.admin_password.is_empty() {
        // No password configured = no auth required (dev mode)
        return true;
    }

    if let Some(cookie) = jar.get(ADMIN_SESSION_COOKIE) {
        // Simple token check: hash of username + password + secret
        let expected_token = generate_session_token(
            &app_state.config.auth.admin_username,
            &app_state.config.auth.admin_password,
            &app_state.config.auth.jwt_secret,
        );
        return cookie.value() == expected_token;
    }
    false
}

/// 🔑 Generate a simple session token
fn generate_session_token(username: &str, password: &str, secret: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    format!("{}:{}:{}", username, password, secret).hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// 🔐 Admin Login Page
pub async fn admin_login(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    // If already authenticated, redirect to dashboard
    if is_admin_authenticated(&jar, &app_state) {
        return Redirect::to("/admin").into_response();
    }

    Html(render_login_page(None)).into_response()
}

/// 🔐 Admin Login POST Handler
pub async fn admin_login_post(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let expected_username = &app_state.config.auth.admin_username;
    let expected_password = &app_state.config.auth.admin_password;

    if form.username == *expected_username && form.password == *expected_password {
        info!("🔓 Admin login successful for user: {}", form.username);

        let token = generate_session_token(
            expected_username,
            expected_password,
            &app_state.config.auth.jwt_secret,
        );

        let cookie = Cookie::build((ADMIN_SESSION_COOKIE, token))
            .path("/admin")
            .http_only(true)
            .secure(app_state.config.is_production())
            .max_age(time::Duration::hours(24))
            .build();

        (jar.add(cookie), Redirect::to("/admin")).into_response()
    } else {
        warn!("🚫 Admin login failed for user: {}", form.username);
        Html(render_login_page(Some("Invalid username or password"))).into_response()
    }
}

/// 🚪 Admin Logout Handler
pub async fn admin_logout(jar: CookieJar) -> impl IntoResponse {
    info!("🚪 Admin logged out");

    let cookie = Cookie::build((ADMIN_SESSION_COOKIE, ""))
        .path("/admin")
        .max_age(time::Duration::seconds(0))
        .build();

    (jar.remove(cookie), Redirect::to("/admin/login")).into_response()
}

/// 🔐 Render login page HTML
fn render_login_page(error: Option<&str>) -> String {
    let error_html = error.map(|e| format!(
        r#"<div class="error-message">{}</div>"#, e
    )).unwrap_or_default();

    format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Admin Login - Feedbacker</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f23;
            color: #cccccc;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .login-container {{
            background: #1a1a2e;
            padding: 40px;
            border-radius: 12px;
            border: 1px solid #333;
            width: 100%;
            max-width: 400px;
        }}
        .login-container h1 {{
            color: #00d4ff;
            text-align: center;
            margin-bottom: 30px;
        }}
        .form-group {{
            margin-bottom: 20px;
        }}
        .form-group label {{
            display: block;
            margin-bottom: 8px;
            color: #888;
        }}
        .form-group input {{
            width: 100%;
            padding: 12px;
            border: 1px solid #333;
            border-radius: 8px;
            background: #0f0f23;
            color: #fff;
            font-size: 16px;
        }}
        .form-group input:focus {{
            outline: none;
            border-color: #00d4ff;
        }}
        .btn {{
            width: 100%;
            padding: 14px;
            background: #00d4ff;
            color: #000;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            font-weight: 600;
            cursor: pointer;
            transition: background 0.2s;
        }}
        .btn:hover {{
            background: #00a8cc;
        }}
        .error-message {{
            background: #3d0000;
            color: #ff4444;
            padding: 12px;
            border-radius: 8px;
            margin-bottom: 20px;
            text-align: center;
        }}
        .back-link {{
            display: block;
            text-align: center;
            margin-top: 20px;
            color: #888;
            text-decoration: none;
        }}
        .back-link:hover {{
            color: #00d4ff;
        }}
    </style>
</head>
<body>
    <div class="login-container">
        <h1>🔐 Admin Login</h1>
        {error_html}
        <form method="POST" action="/admin/login">
            <div class="form-group">
                <label for="username">Username</label>
                <input type="text" id="username" name="username" required autocomplete="username">
            </div>
            <div class="form-group">
                <label for="password">Password</label>
                <input type="password" id="password" name="password" required autocomplete="current-password">
            </div>
            <button type="submit" class="btn">Login</button>
        </form>
        <a href="/" class="back-link">← Back to Site</a>
    </div>
</body>
</html>
"#, error_html = error_html)
}

/// 🔐 Middleware-like function to check auth and redirect if not logged in
fn require_admin_auth(jar: &CookieJar, app_state: &AppState) -> Option<Response> {
    if !is_admin_authenticated(jar, app_state) {
        Some(Redirect::to("/admin/login").into_response())
    } else {
        None
    }
}

/// 📊 Dashboard statistics
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_users: i64,
    pub total_projects: i64,
    pub total_feedback: i64,
    pub pending_feedback: i64,
    pub completed_feedback: i64,
    pub failed_feedback: i64,
}

/// 📋 Feedback item for listing
#[derive(Debug, Serialize)]
pub struct FeedbackItem {
    pub id: String,
    pub repository: String,
    pub status: String,
    pub created_at: String,
    pub content_preview: String,
}

/// 🏠 Admin Dashboard
pub async fn admin_dashboard(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin dashboard accessed");

    let stats = get_dashboard_stats(&app_state).await.unwrap_or(DashboardStats {
        total_users: 0,
        total_projects: 0,
        total_feedback: 0,
        pending_feedback: 0,
        completed_feedback: 0,
        failed_feedback: 0,
    });

    let recent_feedback = get_recent_feedback(&app_state, 10).await.unwrap_or_default();

    Html(format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Admin Dashboard - Feedbacker</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f23;
            color: #cccccc;
            min-height: 100vh;
        }}
        .sidebar {{
            position: fixed;
            left: 0;
            top: 0;
            width: 250px;
            height: 100vh;
            background: #1a1a2e;
            padding: 20px;
            border-right: 1px solid #333;
        }}
        .sidebar h1 {{
            color: #00d4ff;
            font-size: 1.5em;
            margin-bottom: 30px;
            padding-bottom: 20px;
            border-bottom: 1px solid #333;
        }}
        .sidebar nav a {{
            display: block;
            color: #888;
            text-decoration: none;
            padding: 12px 15px;
            margin: 5px 0;
            border-radius: 8px;
            transition: all 0.2s;
        }}
        .sidebar nav a:hover, .sidebar nav a.active {{
            background: #252542;
            color: #00d4ff;
        }}
        .main {{
            margin-left: 250px;
            padding: 30px;
        }}
        .header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 30px;
        }}
        .header h2 {{
            color: #fff;
            font-size: 1.8em;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .stat-card {{
            background: #1a1a2e;
            padding: 25px;
            border-radius: 12px;
            border: 1px solid #333;
        }}
        .stat-card h3 {{
            color: #888;
            font-size: 0.9em;
            margin-bottom: 10px;
        }}
        .stat-card .value {{
            font-size: 2.5em;
            font-weight: bold;
            color: #00d4ff;
        }}
        .stat-card.success .value {{ color: #00ff88; }}
        .stat-card.warning .value {{ color: #ffaa00; }}
        .stat-card.danger .value {{ color: #ff4444; }}
        .card {{
            background: #1a1a2e;
            border-radius: 12px;
            border: 1px solid #333;
            margin-bottom: 20px;
        }}
        .card-header {{
            padding: 20px;
            border-bottom: 1px solid #333;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        .card-header h3 {{
            color: #fff;
        }}
        .card-body {{
            padding: 20px;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th, td {{
            padding: 12px 15px;
            text-align: left;
            border-bottom: 1px solid #333;
        }}
        th {{
            color: #888;
            font-weight: 500;
            font-size: 0.85em;
            text-transform: uppercase;
        }}
        .status {{
            display: inline-block;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: 500;
        }}
        .status-pending {{ background: #3d3d00; color: #ffaa00; }}
        .status-completed {{ background: #003d00; color: #00ff88; }}
        .status-failed {{ background: #3d0000; color: #ff4444; }}
        .status-processing {{ background: #003d3d; color: #00d4ff; }}
        .btn {{
            display: inline-block;
            padding: 10px 20px;
            border-radius: 8px;
            text-decoration: none;
            font-weight: 500;
            transition: all 0.2s;
            border: none;
            cursor: pointer;
        }}
        .btn-primary {{
            background: #00d4ff;
            color: #000;
        }}
        .btn-primary:hover {{
            background: #00a8cc;
        }}
        .empty-state {{
            text-align: center;
            padding: 40px;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin" class="active">📊 Dashboard</a>
            <a href="/admin/feedback">📝 Feedback</a>
            <a href="/admin/projects">🏠 Projects</a>
            <a href="/admin/users">👥 Users</a>
            <a href="/admin/jobs">⚙️ Background Jobs</a>
            <a href="/admin/settings">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>

    <div class="main">
        <div class="header">
            <h2>📊 Dashboard</h2>
            <span style="color: #888;">Welcome, Admin</span>
        </div>

        <div class="stats-grid">
            <div class="stat-card">
                <h3>Total Users</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card">
                <h3>Total Projects</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card">
                <h3>Total Feedback</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card warning">
                <h3>Pending</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card success">
                <h3>Completed</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card danger">
                <h3>Failed</h3>
                <div class="value">{}</div>
            </div>
        </div>

        <div class="card">
            <div class="card-header">
                <h3>📝 Recent Feedback</h3>
                <a href="/admin/feedback" class="btn btn-primary">View All</a>
            </div>
            <div class="card-body">
                {}
            </div>
        </div>
    </div>
</body>
</html>
"#,
        stats.total_users,
        stats.total_projects,
        stats.total_feedback,
        stats.pending_feedback,
        stats.completed_feedback,
        stats.failed_feedback,
        render_feedback_table(&recent_feedback),
    )).into_response()
}

/// 📝 Feedback Management Page
pub async fn admin_feedback(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin feedback page accessed");

    let feedback = get_recent_feedback(&app_state, 50).await.unwrap_or_default();

    Html(format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Feedback Management - Feedbacker Admin</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f23;
            color: #cccccc;
            min-height: 100vh;
        }}
        .sidebar {{
            position: fixed;
            left: 0;
            top: 0;
            width: 250px;
            height: 100vh;
            background: #1a1a2e;
            padding: 20px;
            border-right: 1px solid #333;
        }}
        .sidebar h1 {{ color: #00d4ff; font-size: 1.5em; margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #333; }}
        .sidebar nav a {{ display: block; color: #888; text-decoration: none; padding: 12px 15px; margin: 5px 0; border-radius: 8px; transition: all 0.2s; }}
        .sidebar nav a:hover, .sidebar nav a.active {{ background: #252542; color: #00d4ff; }}
        .main {{ margin-left: 250px; padding: 30px; }}
        .header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; }}
        .header h2 {{ color: #fff; font-size: 1.8em; }}
        .card {{ background: #1a1a2e; border-radius: 12px; border: 1px solid #333; }}
        .card-header {{ padding: 20px; border-bottom: 1px solid #333; }}
        .card-body {{ padding: 20px; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 12px 15px; text-align: left; border-bottom: 1px solid #333; }}
        th {{ color: #888; font-weight: 500; font-size: 0.85em; text-transform: uppercase; }}
        .status {{ display: inline-block; padding: 4px 12px; border-radius: 20px; font-size: 0.85em; font-weight: 500; }}
        .status-pending {{ background: #3d3d00; color: #ffaa00; }}
        .status-completed {{ background: #003d00; color: #00ff88; }}
        .status-failed {{ background: #3d0000; color: #ff4444; }}
        .status-processing {{ background: #003d3d; color: #00d4ff; }}
        .empty-state {{ text-align: center; padding: 40px; color: #666; }}
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin">📊 Dashboard</a>
            <a href="/admin/feedback" class="active">📝 Feedback</a>
            <a href="/admin/projects">🏠 Projects</a>
            <a href="/admin/users">👥 Users</a>
            <a href="/admin/jobs">⚙️ Background Jobs</a>
            <a href="/admin/settings">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>
    <div class="main">
        <div class="header">
            <h2>📝 Feedback Management</h2>
        </div>
        <div class="card">
            <div class="card-header">
                <h3>All Feedback Submissions</h3>
            </div>
            <div class="card-body">
                {}
            </div>
        </div>
    </div>
</body>
</html>
"#, render_feedback_table(&feedback))).into_response()
}

/// 🏠 Projects Management Page
pub async fn admin_projects(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin projects page accessed");

    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Projects - Feedbacker Admin</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f0f23; color: #cccccc; min-height: 100vh; }
        .sidebar { position: fixed; left: 0; top: 0; width: 250px; height: 100vh; background: #1a1a2e; padding: 20px; border-right: 1px solid #333; }
        .sidebar h1 { color: #00d4ff; font-size: 1.5em; margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #333; }
        .sidebar nav a { display: block; color: #888; text-decoration: none; padding: 12px 15px; margin: 5px 0; border-radius: 8px; transition: all 0.2s; }
        .sidebar nav a:hover, .sidebar nav a.active { background: #252542; color: #00d4ff; }
        .main { margin-left: 250px; padding: 30px; }
        .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; }
        .header h2 { color: #fff; font-size: 1.8em; }
        .card { background: #1a1a2e; border-radius: 12px; border: 1px solid #333; padding: 40px; text-align: center; }
        .card p { color: #666; margin-top: 10px; }
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin">📊 Dashboard</a>
            <a href="/admin/feedback">📝 Feedback</a>
            <a href="/admin/projects" class="active">🏠 Projects</a>
            <a href="/admin/users">👥 Users</a>
            <a href="/admin/jobs">⚙️ Background Jobs</a>
            <a href="/admin/settings">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>
    <div class="main">
        <div class="header">
            <h2>🏠 Projects Management</h2>
        </div>
        <div class="card">
            <h3>📋 No projects yet</h3>
            <p>Projects will appear here when users connect their repositories.</p>
        </div>
    </div>
</body>
</html>
"#).into_response()
}

/// 👥 Users Management Page
pub async fn admin_users(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin users page accessed");

    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Users - Feedbacker Admin</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f0f23; color: #cccccc; min-height: 100vh; }
        .sidebar { position: fixed; left: 0; top: 0; width: 250px; height: 100vh; background: #1a1a2e; padding: 20px; border-right: 1px solid #333; }
        .sidebar h1 { color: #00d4ff; font-size: 1.5em; margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #333; }
        .sidebar nav a { display: block; color: #888; text-decoration: none; padding: 12px 15px; margin: 5px 0; border-radius: 8px; transition: all 0.2s; }
        .sidebar nav a:hover, .sidebar nav a.active { background: #252542; color: #00d4ff; }
        .main { margin-left: 250px; padding: 30px; }
        .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; }
        .header h2 { color: #fff; font-size: 1.8em; }
        .card { background: #1a1a2e; border-radius: 12px; border: 1px solid #333; padding: 40px; text-align: center; }
        .card p { color: #666; margin-top: 10px; }
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin">📊 Dashboard</a>
            <a href="/admin/feedback">📝 Feedback</a>
            <a href="/admin/projects">🏠 Projects</a>
            <a href="/admin/users" class="active">👥 Users</a>
            <a href="/admin/jobs">⚙️ Background Jobs</a>
            <a href="/admin/settings">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>
    <div class="main">
        <div class="header">
            <h2>👥 User Management</h2>
        </div>
        <div class="card">
            <h3>👤 No users yet</h3>
            <p>Users will appear here when they register.</p>
        </div>
    </div>
</body>
</html>
"#).into_response()
}

/// ⚙️ Background Jobs Page
pub async fn admin_jobs(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin jobs page accessed");

    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Background Jobs - Feedbacker Admin</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f0f23; color: #cccccc; min-height: 100vh; }
        .sidebar { position: fixed; left: 0; top: 0; width: 250px; height: 100vh; background: #1a1a2e; padding: 20px; border-right: 1px solid #333; }
        .sidebar h1 { color: #00d4ff; font-size: 1.5em; margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #333; }
        .sidebar nav a { display: block; color: #888; text-decoration: none; padding: 12px 15px; margin: 5px 0; border-radius: 8px; transition: all 0.2s; }
        .sidebar nav a:hover, .sidebar nav a.active { background: #252542; color: #00d4ff; }
        .main { margin-left: 250px; padding: 30px; }
        .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; }
        .header h2 { color: #fff; font-size: 1.8em; }
        .card { background: #1a1a2e; border-radius: 12px; border: 1px solid #333; padding: 40px; text-align: center; }
        .card p { color: #666; margin-top: 10px; }
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin">📊 Dashboard</a>
            <a href="/admin/feedback">📝 Feedback</a>
            <a href="/admin/projects">🏠 Projects</a>
            <a href="/admin/users">👥 Users</a>
            <a href="/admin/jobs" class="active">⚙️ Background Jobs</a>
            <a href="/admin/settings">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>
    <div class="main">
        <div class="header">
            <h2>⚙️ Background Jobs</h2>
        </div>
        <div class="card">
            <h3>🔄 No jobs running</h3>
            <p>Background jobs will appear here when processing feedback.</p>
        </div>
    </div>
</body>
</html>
"#).into_response()
}

/// 🔧 Settings Page
pub async fn admin_settings(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> Response {
    if let Some(redirect) = require_admin_auth(&jar, &app_state) {
        return redirect;
    }
    info!("🔧 Admin settings page accessed");

    Html(format!(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Settings - Feedbacker Admin</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f0f23; color: #cccccc; min-height: 100vh; }}
        .sidebar {{ position: fixed; left: 0; top: 0; width: 250px; height: 100vh; background: #1a1a2e; padding: 20px; border-right: 1px solid #333; }}
        .sidebar h1 {{ color: #00d4ff; font-size: 1.5em; margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #333; }}
        .sidebar nav a {{ display: block; color: #888; text-decoration: none; padding: 12px 15px; margin: 5px 0; border-radius: 8px; transition: all 0.2s; }}
        .sidebar nav a:hover, .sidebar nav a.active {{ background: #252542; color: #00d4ff; }}
        .main {{ margin-left: 250px; padding: 30px; }}
        .header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; }}
        .header h2 {{ color: #fff; font-size: 1.8em; }}
        .card {{ background: #1a1a2e; border-radius: 12px; border: 1px solid #333; margin-bottom: 20px; }}
        .card-header {{ padding: 20px; border-bottom: 1px solid #333; }}
        .card-header h3 {{ color: #fff; }}
        .card-body {{ padding: 20px; }}
        .setting-row {{ display: flex; justify-content: space-between; align-items: center; padding: 15px 0; border-bottom: 1px solid #333; }}
        .setting-row:last-child {{ border-bottom: none; }}
        .setting-label {{ color: #fff; }}
        .setting-value {{ color: #00d4ff; font-family: monospace; }}
        .setting-status {{ padding: 4px 12px; border-radius: 20px; font-size: 0.85em; }}
        .status-ok {{ background: #003d00; color: #00ff88; }}
        .status-warn {{ background: #3d3d00; color: #ffaa00; }}
    </style>
</head>
<body>
    <div class="sidebar">
        <h1>🚢 Feedbacker</h1>
        <nav>
            <a href="/admin">📊 Dashboard</a>
            <a href="/admin/feedback">📝 Feedback</a>
            <a href="/admin/projects">🏠 Projects</a>
            <a href="/admin/users">👥 Users</a>
            <a href="/admin/jobs">⚙️ Background Jobs</a>
            <a href="/admin/settings" class="active">🔧 Settings</a>
            <a href="/">← Back to Site</a>
            <a href="/admin/logout" style="margin-top: 30px; color: #ff4444;">🚪 Logout</a>
        </nav>
    </div>
    <div class="main">
        <div class="header">
            <h2>🔧 Settings</h2>
        </div>

        <div class="card">
            <div class="card-header">
                <h3>🐙 GitHub Integration</h3>
            </div>
            <div class="card-body">
                <div class="setting-row">
                    <span class="setting-label">GitHub Username</span>
                    <span class="setting-value">{}</span>
                </div>
                <div class="setting-row">
                    <span class="setting-label">GitHub Token</span>
                    <span class="setting-status status-ok">✓ Configured</span>
                </div>
            </div>
        </div>

        <div class="card">
            <div class="card-header">
                <h3>🤖 LLM Providers</h3>
            </div>
            <div class="card-body">
                <div class="setting-row">
                    <span class="setting-label">OpenAI</span>
                    <span class="setting-status {}">{}</span>
                </div>
                <div class="setting-row">
                    <span class="setting-label">Anthropic</span>
                    <span class="setting-status {}">{}</span>
                </div>
                <div class="setting-row">
                    <span class="setting-label">Default Provider</span>
                    <span class="setting-value">{:?}</span>
                </div>
            </div>
        </div>

        <div class="card">
            <div class="card-header">
                <h3>🚦 Rate Limiting</h3>
            </div>
            <div class="card-body">
                <div class="setting-row">
                    <span class="setting-label">Requests per Minute</span>
                    <span class="setting-value">{}</span>
                </div>
                <div class="setting-row">
                    <span class="setting-label">Feedback per Hour</span>
                    <span class="setting-value">{}</span>
                </div>
            </div>
        </div>
    </div>
</body>
</html>
"#,
        app_state.config.github.username,
        if app_state.config.llm.openai.is_some() { "status-ok" } else { "status-warn" },
        if app_state.config.llm.openai.is_some() { "✓ Configured" } else { "⚠ Not configured" },
        if app_state.config.llm.anthropic.is_some() { "status-ok" } else { "status-warn" },
        if app_state.config.llm.anthropic.is_some() { "✓ Configured" } else { "⚠ Not configured" },
        app_state.config.llm.default_provider,
        app_state.config.rate_limiting.requests_per_minute,
        app_state.config.rate_limiting.feedback_per_hour,
    )).into_response()
}

// Helper functions

async fn get_dashboard_stats(app_state: &AppState) -> anyhow::Result<DashboardStats> {
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    let total_projects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    let total_feedback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    let pending_feedback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback WHERE status = 'pending'")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    let completed_feedback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback WHERE status = 'completed'")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    let failed_feedback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feedback WHERE status = 'failed'")
        .fetch_one(&app_state.db_pool)
        .await
        .unwrap_or(0);

    Ok(DashboardStats {
        total_users,
        total_projects,
        total_feedback,
        pending_feedback,
        completed_feedback,
        failed_feedback,
    })
}

async fn get_recent_feedback(app_state: &AppState, limit: i64) -> anyhow::Result<Vec<FeedbackItem>> {
    let rows = sqlx::query(
        "SELECT id, repository, status::text, created_at, content FROM feedback ORDER BY created_at DESC LIMIT $1"
    )
    .bind(limit)
    .fetch_all(&app_state.db_pool)
    .await?;

    let items = rows
        .iter()
        .map(|row| {
            let content: String = row.get("content");
            FeedbackItem {
                id: row.get::<uuid::Uuid, _>("id").to_string(),
                repository: row.get("repository"),
                status: row.get("status"),
                created_at: row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").format("%Y-%m-%d %H:%M").to_string(),
                content_preview: content.chars().take(50).collect::<String>() + if content.len() > 50 { "..." } else { "" },
            }
        })
        .collect();

    Ok(items)
}

fn render_feedback_table(feedback: &[FeedbackItem]) -> String {
    if feedback.is_empty() {
        return r#"<div class="empty-state">📭 No feedback yet</div>"#.to_string();
    }

    let rows: String = feedback
        .iter()
        .map(|f| {
            let status_class = match f.status.as_str() {
                "pending" => "status-pending",
                "completed" => "status-completed",
                "failed" => "status-failed",
                _ => "status-processing",
            };
            format!(
                r#"<tr>
                    <td><code>{}</code></td>
                    <td>{}</td>
                    <td><span class="status {}">{}</span></td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                &f.id[..8],
                f.repository,
                status_class,
                f.status,
                f.created_at,
                f.content_preview,
            )
        })
        .collect();

    format!(
        r#"<table>
            <thead>
                <tr>
                    <th>ID</th>
                    <th>Repository</th>
                    <th>Status</th>
                    <th>Created</th>
                    <th>Content</th>
                </tr>
            </thead>
            <tbody>{}</tbody>
        </table>"#,
        rows
    )
}

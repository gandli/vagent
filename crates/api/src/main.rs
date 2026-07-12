//! vagent-api:本地管理 API(127.0.0.1 loopback)。
//! 与 CLI/bot 共享同一套 core;handler 逻辑抽成 handlers.rs 纯函数便于单测。
//! 鉴权:VAGENT_API_TOKEN 配置后所有 /api/* 须 Bearer token;未配置时只读放行、写拒绝。

mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{Html, Json, Response},
    routing::{get, post},
    Router,
};
use serde_json::json;
use vagent_core::{load_spec, save_spec, Protocol};

/// 共享状态:配置路径 + 可选 API token。
pub struct AppState {
    config: PathBuf,
    token: Option<String>,
}

type SharedState = Arc<AppState>;

fn default_config() -> PathBuf {
    vagent_core::Spec::default_config_path()
}

#[tokio::main]
async fn main() {
    let config = std::env::var("VAGENT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config());
    let token = std::env::var("VAGENT_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());

    if token.is_none() {
        eprintln!(
            "警告: 未设置 VAGENT_API_TOKEN,写操作(POST)将被拒绝。设置该环境变量以启用完整 API。"
        );
    }

    let state = Arc::new(AppState { config, token });

    let app = router(state);

    let listener = match tokio::net::TcpListener::bind("127.0.0.1:7800").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("绑定 127.0.0.1:7800 失败: {e}（端口可能被占用或权限不足）");
            return;
        }
    };
    println!("vagent-api listening on http://127.0.0.1:7800");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("server error: {e}");
    }
}

/// 构造路由(导出以便将来做 oneshot 测试)。
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/status", get(api_status))
        .route("/api/render", get(api_render))
        .route("/api/users", post(api_add_user))
        .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .with_state(state)
}

/// 鉴权中间件:根据方法(读/写)与配置 token 判定,拒绝返回 401/403。
async fn auth_layer(
    State(state): State<SharedState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let is_write = !req.method().is_safe();
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if handlers::is_authorized(is_write, state.token.as_deref(), auth_header) {
        Ok(next.run(req).await)
    } else if state.token.is_some() {
        Err(StatusCode::UNAUTHORIZED)
    } else {
        // 未配置 token 且为写操作
        Err(StatusCode::FORBIDDEN)
    }
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

async fn api_status(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(handlers::status_view(&spec)))
}

async fn api_render(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use vagent_core::render::xray;
    use vagent_core::spec::Spec;
    let spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    let out = xray::render(&spec, &Spec::base_dir(&state.config))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(out))
}

#[derive(serde::Deserialize)]
struct AddUserBody {
    name: String,
    #[serde(default = "default_port")]
    port: u16,
    /// 是否 Reality 用户(默认 false)。Reality 用户需后续生成密钥。
    #[serde(default)]
    reality: bool,
}

fn default_port() -> u16 {
    443
}

async fn api_add_user(
    State(state): State<SharedState>,
    Json(body): Json<AddUserBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut spec = load_spec(&state.config).map_err(|_| StatusCode::NOT_FOUND)?;
    // 端口唯一性:重复端口会导致 xray 绑定冲突(对齐 CLI R10 校验)
    if spec.users.iter().any(|u| u.port == body.port) {
        return Err(StatusCode::CONFLICT);
    }
    // 不再硬编码 reality=true;按请求 reality 标志(默认普通 vless)
    spec.add_user(&body.name, Protocol::Vless, body.port, body.reality);
    save_spec(&spec, &state.config).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        json!({ "ok": true, "name": body.name, "port": body.port, "reality": body.reality }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt; // oneshot

    fn state_with(token: Option<&str>) -> SharedState {
        // 用 tempfile::tempdir 保证目录存在且唯一(避免对 temp_dir() 返回值的隐含依赖)
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let cfg = tmp.path().join("spec.toml");
        let spec = vagent_core::Spec::default_for("t.example.com");
        vagent_core::save_spec(&spec, &cfg).unwrap();
        // 持有 tmp 防止目录在测试期间被析构删除
        std::mem::forget(tmp);
        Arc::new(AppState {
            config: cfg,
            token: token.map(|s| s.to_string()),
        })
    }

    #[tokio::test]
    async fn get_status_allowed_with_valid_token() {
        let app = router(state_with(Some("tok")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header("authorization", "Bearer tok")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_status_without_token_when_configured_is_401() {
        // token 已配置但请求不带 → 401(读操作也不放行)
        let app = router(state_with(Some("tok")));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn post_user_forbidden_without_token() {
        let app = router(state_with(None));
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/users")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"bob","port":8443}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "无 token 写操作应 403"
        );
    }

    #[tokio::test]
    async fn post_user_unauthorized_with_wrong_token() {
        let app = router(state_with(Some("s3cret")));
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/users")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer wrong")
                    .body(Body::from(r#"{"name":"bob","port":8443}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "错误 token 应 401");
    }

    #[tokio::test]
    async fn post_user_ok_with_valid_token_and_no_hardcoded_reality() {
        let app = router(state_with(Some("s3cret")));
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/users")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer s3cret")
                    .body(Body::from(r#"{"name":"bob","port":8443}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ok"], serde_json::json!(true));
        // 关键:不再硬编码 reality=true
        assert_eq!(
            v["reality"],
            serde_json::json!(false),
            "默认不应是 reality 用户"
        );
    }
}

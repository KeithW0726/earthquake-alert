use crate::db::Database;
use crate::event_cache::EventCache;
use crate::models::{ApiResponse, CachedEvent, SubscribeRequest, Subscription};
use crate::utils::{distance, intensity};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub event_cache: EventCache,
    pub bark_api_url: String,
}

/// 订阅处理器
pub async fn subscribe_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubscribeRequest>,
) -> impl IntoResponse {
    // 验证输入
    if payload.bark_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error("Bark ID 不能为空")),
        );
    }

    // Bark ID 长度限制，防止过长数据
    if payload.bark_id.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "Bark ID 过长（最大64字符）",
            )),
        );
    }

    // 验证 Bark ID 只包含安全字符（字母、数字）
    if !payload.bark_id.chars().all(|c| c.is_alphanumeric()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "Bark ID 只能包含字母、数字",
            )),
        );
    }

    if !distance::validate_coordinates(payload.latitude, payload.longitude) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error("无效的经纬度坐标")),
        );
    }

    if !intensity::validate_intensity(payload.min_intensity) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<SubscribeResponse>::error(
                "烈度阈值必须在 0-7 之间",
            )),
        );
    }

    // 创建订阅
    let subscription = Subscription::new(
        payload.bark_id.clone(),
        payload.latitude,
        payload.longitude,
        payload.min_intensity,
        payload.bark_api_url.clone(),
        payload.passive_max,
        payload.active_max,
    );

    // 打印订阅信息
    tracing::info!(
        "收到订阅请求 - Bark ID: {}, 位置: ({:.4}, {:.4}), 最小震度: {}",
        subscription.bark_id,
        subscription.latitude,
        subscription.longitude,
        subscription.min_intensity
    );

    // 保存到数据库
    let store = state.db.subscriptions();
    match store.upsert_subscription(subscription.clone()) {
        Ok(_) => {
            tracing::info!(
                "订阅成功 - Bark ID: {}, GeoHash: {}",
                subscription.bark_id,
                crate::utils::geohash::encode(subscription.latitude, subscription.longitude)
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(
                    "订阅成功",
                    Some(SubscribeResponse::from(subscription)),
                )),
            )
        }
        Err(e) => {
            tracing::error!(
                "订阅失败 - Bark ID: {}, 错误: {:?}",
                subscription.bark_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<SubscribeResponse>::error(format!(
                    "订阅失败: {}",
                    e
                ))),
            )
        }
    }
}

/// 取消订阅处理器（路径参数版本）
pub async fn unsubscribe_by_path_handler(
    State(state): State<AppState>,
    Path(bark_id): Path<String>,
) -> impl IntoResponse {
    // 验证输入
    if bark_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Bark ID 不能为空")),
        );
    }

    // Bark ID 长度限制，防止过长数据
    if bark_id.len() > 256 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("Bark ID 过长（最大256字符）")),
        );
    }

    // 验证 Bark ID 只包含安全字符（字母、数字、下划线、连字符）
    if !bark_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                "Bark ID 只能包含字母、数字、下划线和连字符",
            )),
        );
    }

    tracing::info!("收到取消订阅请求（路径参数）- Bark ID: {}", bark_id);

    let store = state.db.subscriptions();
    match store.delete_subscription(&bark_id) {
        Ok(_) => {
            tracing::info!("取消订阅成功 - Bark ID: {}", bark_id);
            (
                StatusCode::OK,
                Json(ApiResponse::<()>::success("已取消订阅", None)),
            )
        }
        Err(e) => {
            tracing::error!("取消订阅失败 - Bark ID: {}, 错误: {:?}", bark_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(format!("取消订阅失败: {}", e))),
            )
        }
    }
}

/// 订阅成功响应
#[derive(Serialize)]
pub struct SubscribeResponse {
    pub bark_id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub min_intensity: u8,
    pub bark_api_url: String,
    pub passive_max: u8,
    pub active_max: u8,
    pub created_at: i64,
}

impl From<Subscription> for SubscribeResponse {
    fn from(sub: Subscription) -> Self {
        Self {
            bark_id: sub.bark_id,
            latitude: sub.latitude,
            longitude: sub.longitude,
            min_intensity: sub.min_intensity,
            bark_api_url: sub.bark_api_url,
            passive_max: sub.passive_max,
            active_max: sub.active_max,
            created_at: sub.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_subscriptions: usize,
}

/// 获取统计信息
pub async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.db.subscriptions();
    match store.get_total_count() {
        Ok(count) => (
            StatusCode::OK,
            Json(ApiResponse::success(
                "统计成功",
                Some(StatsResponse {
                    total_subscriptions: count,
                }),
            )),
        ),
        Err(e) => {
            tracing::error!("获取统计失败: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<StatsResponse>::error(format!(
                    "获取统计失败: {}",
                    e
                ))),
            )
        }
    }
}

/// 历史查询参数
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub source: Option<String>,
    pub min_mag: Option<f64>,
    pub page: Option<usize>,
}

/// 历史事件查询响应
#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub events: Vec<CachedEvent>,
    pub total_pages: usize,
    pub page: usize,
}

/// 获取历史地震事件
pub async fn history_handler(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let min_mag = params.min_mag.unwrap_or(0.0);
    let source = params.source.as_deref();

    let (events, total_pages) = state
        .event_cache
        .list(source, min_mag, page, 10)
        .await;

    (
        StatusCode::OK,
        Json(ApiResponse::success(
            "ok",
            Some(HistoryResponse {
                events,
                total_pages,
                page,
            }),
        )),
    )
}

/// 测试通知请求
#[derive(Debug, Deserialize)]
pub struct TestRequest {
    pub bark_id: String,
    pub event_id: String,
}

/// 测试通知响应
#[derive(Debug, Serialize)]
pub struct TestResponse {
    pub level: String,
    pub estimated_intensity: u8,
    pub distance_km: f64,
    pub magnitude: f64,
    pub region: String,
}

/// 用历史数据发送测试通知
pub async fn test_notification_handler(
    State(state): State<AppState>,
    Json(payload): Json<TestRequest>,
) -> impl IntoResponse {
    let store = state.db.subscriptions();

    let subscription = match store.get_subscription(&payload.bark_id) {
        Ok(sub) => sub,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<TestResponse>::error("订阅不存在，请先订阅")),
            );
        }
    };

    let event = match state.event_cache.get_by_id(&payload.event_id).await {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<TestResponse>::error("事件不存在或已过期")),
            );
        }
    };

    // 计算距离和预估烈度
    let dist = distance::vincenty_distance(
        event.latitude,
        event.longitude,
        subscription.latitude,
        subscription.longitude,
    )
    .unwrap_or(0.0);

    let estimated_intensity = intensity::estimate_intensity(event.magnitude, dist);

    if estimated_intensity < subscription.min_intensity {
        return (
            StatusCode::OK,
            Json(ApiResponse::success(
                "测试未达到推送阈值，未发送通知",
                Some(TestResponse {
                    level: String::new(),
                    estimated_intensity,
                    distance_km: dist,
                    magnitude: event.magnitude,
                    region: event.region.clone(),
                }),
            )),
        );
    }

    // 确定通知级别
    let (level, level_params) =
        if estimated_intensity <= subscription.passive_max {
            ("passive", "level=passive")
        } else if estimated_intensity <= subscription.active_max {
            ("active", "level=active&volume=5")
        } else {
            ("critical", "level=critical&volume=10&call=1")
        };

    // 构建通知内容
    let title = format!("地震预警 M{:.1} (测试)", event.magnitude);
    let subtitle = format!("震度 {} 级 · 距离 {:.1} km", estimated_intensity, dist);
    let body = format!(
        "震央: {}\n震源深度: {:.0} km\n最大震度: {} 级\n\n* 此为历史数据测试通知 *",
        event.region, event.depth, event.max_intensity,
    );

    let base_url = if subscription.bark_api_url.is_empty() {
        &state.bark_api_url
    } else {
        &subscription.bark_api_url
    };

    let url = format!(
        "{}/{}/{}/{}/{}?group=地震预警测试&{}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(&payload.bark_id),
        urlencoding::encode(&title),
        urlencoding::encode(&subtitle),
        urlencoding::encode(&body),
        level_params,
    );

    // 发送通知
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                "测试通知发送成功: bark_id={}, event={}, level={}",
                payload.bark_id,
                payload.event_id,
                level
            );
            (
                StatusCode::OK,
                Json(ApiResponse::success(
                    format!("测试通知已发送（级别: {}）", level),
                    Some(TestResponse {
                        level: level.to_string(),
                        estimated_intensity,
                        distance_km: dist,
                        magnitude: event.magnitude,
                        region: event.region.clone(),
                    }),
                )),
            )
        }
        Ok(resp) => {
            let status = resp.status();
            (
                StatusCode::OK,
                Json(ApiResponse::<TestResponse>::error(format!(
                    "Bark 服务器返回错误: {}",
                    status
                ))),
            )
        }
        Err(e) => (
            StatusCode::OK,
            Json(ApiResponse::<TestResponse>::error(format!(
                "网络请求失败: {}",
                e
            ))),
        ),
    }
}

/// 获取单个订阅信息
pub async fn get_subscription_handler(
    State(state): State<AppState>,
    Path(bark_id): Path<String>,
) -> impl IntoResponse {
    let store = state.db.subscriptions();
    match store.get_subscription(&bark_id) {
        Ok(sub) => (
            StatusCode::OK,
            Json(ApiResponse::success("ok", Some(SubscribeResponse::from(sub)))),
        ),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SubscribeResponse>::error("订阅不存在")),
        ),
    }
}

/// 健康检查
pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(ApiResponse::<()>::success("OK", None)))
}

# 地震预警 Bark 订阅系统

基于 [github.com/noctiro/earthquake-alert](https://github.com/noctiro/earthquake-alert) 并参考 [https://eew.saevio.top/](https://eew.saevio.top/) 的单二进制地震预警推送服务，可运行在 Windows、Linux。通过 Bark App 实时推送，内置 Web 管理页面。

## 使用方式

前往 GitHub 仓库的 Actions 页面，下载最新构建的 Windows 或 Linux 二进制文件，直接运行即可：

```bash
# Linux
./earthquake-alert-backend

# Windows
earthquake-alert-backend.exe
```

默认监听 `0.0.0.0:30010`，打开浏览器访问。

## 自行构建

需要 Rust 环境和一台服务器：

```bash
cd backend
cargo build --release
./target/release/earthquake-alert-backend
```

## 技术栈

- **后端**: Rust, Axum, sled (嵌入式数据库)
- **前端**: 内置于二进制的单 HTML 页面，高德地图 (GCJ-02 坐标系)
- **推送**: Bark App (支持订阅者自定义推送服务器)

## 相较于原项目的优化

- **单二进制部署** — 移除 Cloudflare Worker，前端页面编译进 Rust 二进制，运行即开
- **高德地图** — 替换 CartoCDN 为高德地图（GCJ-02），中国用户直接可用
- **自定义 Bark 服务器** — 订阅者可设置自己的 `bark_api_url`，不依赖全局配置
- **三级推送阈值** — passive/active/critical 三级可调，用户自定义 `passive_max` 和 `active_max`
- **内置测试通知** — Web 页面直接预览烈度并发送测试 Bark 推送
- **订阅状态页面** — 在线查看/管理订阅信息，无需 API 工具
- **后台缓存** — wolfx API 每 5 分钟自动缓存，减少外部请求延迟
- **跨平台 CI** — GitHub Actions 自动构建 Windows / Linux 二进制，下载即用

## 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `SERVER_HOST` | `0.0.0.0` | 监听地址 |
| `SERVER_PORT` | `30010` | 服务端口 |
| `DB_PATH` | `./data/earthquake.db` | 数据库路径 |
| `BARK_API_URL` | `https://api.day.app` | 默认 Bark 推送服务器 |
| `MAX_CONCURRENT_NOTIFICATIONS` | `1000` | 最大并发推送数 |
| `BATCH_SIZE` | `5000` | 每批处理订阅数 |
| `HTTP_POOL_SIZE` | `200` | HTTP 连接池大小 |

## API 接口

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/health` | 健康检查 |
| `POST` | `/api/subscribe` | 订阅地震预警 |
| `DELETE` | `/api/unsubscribe/{bark_id}` | 取消订阅 |
| `GET` | `/api/stats` | 订阅统计 |
| `GET` | `/api/subscription/{bark_id}` | 查询订阅详情 |
| `GET` | `/api/test-earthquake` | 获取缓存的最新地震数据 |
| `POST` | `/api/test-notify` | 发送测试通知 |

### 订阅请求

```json
{
  "bark_id": "your-bark-key",
  "latitude": 30.5,
  "longitude": 104.0,
  "min_intensity": 3,
  "passive_max": 1,
  "active_max": 2,
  "bark_api_url": "https://api.day.app"
}
```

### 三级推送阈值

- **passive** (烈度 ≤ passive_max): 静默通知, `level=passive`
- **active** (passive_max < 烈度 ≤ active_max): 有声音, `level=active&volume=5`
- **critical** (烈度 > active_max): 高优先级+语音呼叫, `level=critical&volume=10&call=1&sound=Alert`

=========

# 地震预警 Bark 订阅系统

基于 Rust 后端 + Cloudflare Workers 的地震预警实时推送服务。使用 GeoHash 空间索引实现匹配，通过 Bark App 实时推送。

示例: [http://eew.noctiro.moe](http://eew.noctiro.moe)

## 技术栈

* **后端**: Rust, Axum, sled (DB), tokio-tungstenite (WS)
* **前端**: Cloudflare Workers, 原生 JS/HTML, CartoCDN (地图)

## 部署

### 1. 后端部署 (Rust)

需要 Rust 环境和一台服务器。

```bash
cd backend

# 配置环境
cp .env.example .env
# 编辑 .env 修改 SERVER_PORT 或 BARK_API_URL 等

# 构建与运行
cargo build --release
./target/release/earthquake-alert-backend

```

### 2. 前端部署 (Cloudflare Workers)

需要 Node.js 和 Wrangler CLI。

```bash
cd worker

# 编辑 wrangler.toml 配置后端地址
# [vars]
# BACKEND_URL = "http://your-backend-ip:30010"

# 部署
wrangler deploy --env production

```

## 配置说明

### 后端环境变量 (.env)

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `SERVER_HOST` | `0.0.0.0` | 监听地址 |
| `SERVER_PORT` | `30010` | 服务端口 |
| `DB_PATH` | `./data/earthquake.db` | 数据库路径 |
| `BARK_API_URL` | `https://api.day.app` | Bark 服务器地址 |

## 后端 API 接口

* **订阅**: `POST /api/subscribe`
```json
{ "bark_id": "key", "latitude": 35.6, "longitude": 139.6, "min_intensity": 3 }

```


* **退订**: `DELETE /api/unsubscribe/{bark_id}`
* **状态**: `GET /health`
* **统计**: `GET /api/stats`

## 致谢

* 数据源：[wolfx.jp](https://ws-api.wolfx.jp)
* 推送服务：[Bark](https://github.com/Finb/Bark)

// 导入必要的库和模块
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use actix::*;
use actix_files;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, Responder, dev::Server, web};
use actix_cors::Cors;

use crate::modules::config::config::get_config;
use crate::modules::proxy::proxy_http::ProxyMiddleware;
use crate::modules::web::chat_api::upload_media as chat_upload_media;
use crate::modules::web::database::Database;
use crate::modules::web::login_handler::{
    get_user_info, get_user_theme_config_handler, health_check, login, refresh_token,
    update_user_theme_config_handler, get_user_terminal_config_handler, update_user_terminal_config_handler,
};
use crate::modules::web::sftp_api::{
    create_dir as sftp_mkdir, create_session as sftp_create_session, delete_file as sftp_delete,
    download_file as sftp_download, list_dir as sftp_list, read_file as sftp_read,
    rename_file as sftp_rename, set_permissions as sftp_chmod, upload_file as sftp_upload,
    write_file as sftp_write,
};
use crate::modules::web::sqlite_api::{
    add_column, batch_delete_rows, create_table, delete_row, drop_column, drop_table,
    get_all_databases, get_table_data, get_tables_by_database, insert_row, rename_column,
    rename_table, sql_query, update_row,
};
use crate::modules::web::ssh_servers_api::{
    create_group, create_server, delete_group, delete_server, list_groups, list_servers,
    update_group, update_server,
};
use crate::modules::web::ssh_monitor_api;
use actix_web_actors::ws;
// 导入日志宏
// use crate::log_debug;
// use crate::log_error;
use crate::log_info;
use crate::modules::aigateway::tianyi_api::tianyi_ws_route; // 导入天翼云AI WebSocket路由函数
use crate::modules::sftp::service::SftpService; // 导入SftpService
use crate::modules::sqlstudio::{
    create_connection_handler, get_metadata_handler, list_connections_handler,
    test_connection_handler, update_connection_handler, delete_connection_handler,
};
use crate::modules::ssh::SshService; // 从ssh模块导入SshService
use crate::modules::task::api::{start_task, stop_task, task_status};
use crate::modules::task::service::TaskManager;
use crate::modules::web::ChatServer; // 来自 actors.rs 的重导出
use crate::modules::web::apitest_api::{
    create_endpoint as apitest_create, delete_endpoint as apitest_delete,
    get_endpoint as apitest_get, list_endpoints as apitest_list, update_endpoint as apitest_update,
};
use crate::modules::web::auth_middleware::AuthMiddleware;
use crate::modules::web::sobel_ws::sobel_ws_route; // 导入Sobel WebSocket路由函数
use crate::modules::web::ssh_websocket::ssh_route; // 导入SSH WebSocket路由函数
use crate::modules::web::ssh_websocket_pty::ssh_pty_route; // 导入SSH PTY WebSocket路由函数
use crate::modules::web::template_engine::render_with_includes; // 模板引擎 // 导入 AuthMiddleware
// use crate::log_warn;
/// Entry point for our websocket route
async fn chat_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<ChatServer>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        crate::modules::web::WsChatSession {
            id: 0,
            hb: Instant::now(),
            room: "main".to_owned(),
            name: None,
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}
// Displays state
async fn get_count(count: web::Data<AtomicUsize>) -> impl Responder {
    let current_count = count.load(Ordering::SeqCst);
    format!("Visitors: {current_count}")
}

// 配置Actix-Web服务器
pub async fn setup_actix() -> Result<Server, std::io::Error> {
    // 读取配置文件
    let config = get_config().expect("Failed to load configuration");

    // 初始化数据库连接
    let db = Arc::new(
        Database::new(&config.server.database_path).expect("Failed to initialize database"),
    );
    // set up applications state
    // keep a count of the number of visitors
    let app_state = Arc::new(AtomicUsize::new(0));
    // start chat server actor
    let server = ChatServer::new(app_state.clone(), db.clone()).start();

    // 复制配置用于闭包中使用
    let config_clone = config.clone();
    let db_clone = db.clone();

    // 从配置文件中获取WebSocket配置
    let ws_config = config.websocket.clone();

    // 打印WebSocket配置信息
    if ws_config.enabled {
        log_info!(
            "🔌 WebSocket服务已启用路径: {}, 心跳间隔: {}秒",
            ws_config.path,
            ws_config.heartbeat_interval
        );
    } else {
        log_info!("🔌 WebSocket服务已禁用");
    }

    // 创建SSH服务
    let ssh_service = std::sync::Arc::new(std::sync::Mutex::new(SshService::new()));
    let ssh_service_data = web::Data::new(ssh_service.clone());
    // 创建SFTP服务
    let sftp_service = std::sync::Arc::new(tokio::sync::Mutex::new(SftpService::new()));
    let sftp_service_data = web::Data::new(sftp_service.clone());
    // 创建并配置Actix-Web服务器
    let server = HttpServer::new(move || {
        App::new()
            // 提高Json负载大小限制以支持媒体上传（默认较小）
            .app_data(actix_web::web::JsonConfig::default().limit(50 * 1024 * 1024))
            // 全局 CORS（允许前端开发端口访问）
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    .allowed_headers(vec![
                        actix_web::http::header::CONTENT_TYPE,
                        actix_web::http::header::AUTHORIZATION,
                    ])
                    .max_age(3600),
            )
            // 注册应用数据（用于在处理函数中访问）
            .app_data(web::Data::new(db_clone.clone()))
            .app_data(web::Data::new(config_clone.clone()))
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(server.clone()))
            .app_data(ssh_service_data.clone())
            .app_data(sftp_service_data.clone())
            // 任务管理器共享状态
            .app_data(web::Data::new(TaskManager::new()))
            // 注册代理中间件
            .wrap(actix_web::middleware::Logger::default())
            .wrap(ProxyMiddleware::new())
            // 配置API路由
            .route("/count", web::get().to(get_count))
            .route("/ws", web::get().to(chat_route))
            .route("/ws/ssh", web::get().to(ssh_route))
            .route("/ws/ssh-pty", web::get().to(ssh_pty_route))
            .route("/ws/sobel", web::get().to(sobel_ws_route))
            .route("/ws/tianyi", web::get().to(tianyi_ws_route))
            .route("/api/health", web::get().to(health_check))
            .route("/api/login", web::post().to(login))
            .service(
                web::resource("/auth/getUserInfo")
                    .wrap(AuthMiddleware::new())
                    .route(web::get().to(get_user_info)),
            )
            .route("/auth/refreshToken", web::post().to(refresh_token))
            .service(
                web::scope("/api/user")
                    .wrap(AuthMiddleware::new())
                    .route(
                        "/theme-config",
                        web::get().to(get_user_theme_config_handler),
                    )
                    .route(
                        "/theme-config",
                        web::post().to(update_user_theme_config_handler),
                    )
                    .route(
                        "/terminal-config",
                        web::get().to(get_user_terminal_config_handler),
                    )
                    .route(
                        "/terminal-config",
                        web::post().to(update_user_terminal_config_handler),
                    ),
            )
            // SQLite API routes
            .route("/api/sqlite/databases", web::get().to(get_all_databases))
            .route("/api/sqlite/tables", web::get().to(get_tables_by_database))
            .route("/api/sqlite/table-data", web::get().to(get_table_data))
            // 新增SQLite管理API路由
            .route("/api/sqlite/table/create", web::post().to(create_table))
            .route("/api/sqlite/table/delete", web::post().to(drop_table))
            .route("/api/sqlite/table/rename", web::post().to(rename_table))
            .route("/api/sqlite/column/rename", web::post().to(rename_column))
            .route("/api/sqlite/column/add", web::post().to(add_column))
            .route("/api/sqlite/column/drop", web::post().to(drop_column))
            .route("/api/sqlite/row/insert", web::post().to(insert_row))
            .route("/api/sqlite/row/update", web::post().to(update_row))
            .route("/api/sqlite/row/delete", web::post().to(delete_row))
            .route(
                "/api/sqlite/row/batch-delete",
                web::post().to(batch_delete_rows),
            )
            .route("/api/sqlite/query", web::post().to(sql_query))
            // SQL Studio Connections API
            .route(
                "/api/sqlstudio/connection/test",
                web::post().to(test_connection_handler),
            )
            .route(
                "/api/sqlstudio/connection/create",
                web::post().to(create_connection_handler),
            )
            .route(
                "/api/sqlstudio/connection/list",
                web::get().to(list_connections_handler),
            )
            .route(
                "/api/sqlstudio/connection/update",
                web::post().to(update_connection_handler),
            )
            .route(
                "/api/sqlstudio/connection/delete",
                web::post().to(delete_connection_handler),
            )
            .route(
                "/api/sqlstudio/connection/metadata",
                web::post().to(get_metadata_handler),
            )
            // Chat 媒体上传 API 路由
            .route("/api/chat/upload", web::post().to(chat_upload_media))
            // SFTP 文件操作 API 路由（远程SFTP会话）
            .route("/api/sftp/session", web::post().to(sftp_create_session))
            .route("/api/sftp/list", web::get().to(sftp_list))
            .route("/api/sftp/read", web::get().to(sftp_read))
            .route("/api/sftp/write", web::post().to(sftp_write))
            .route("/api/sftp/delete", web::post().to(sftp_delete))
            .route("/api/sftp/rename", web::post().to(sftp_rename))
            .service(
                web::resource("/api/sftp/upload")
                    .app_data(web::JsonConfig::default().limit(100 * 1024 * 1024)) // 100MB limit
                    .route(web::post().to(sftp_upload)),
            )
            .route("/api/sftp/download", web::get().to(sftp_download))
            .route("/api/sftp/mkdir", web::post().to(sftp_mkdir))
            .route("/api/sftp/chmod", web::post().to(sftp_chmod))
            // SSH 服务器配置 CRUD API 路由
            .route("/api/ssh/groups", web::get().to(list_groups))
            .route("/api/ssh/groups", web::post().to(create_group))
            .route("/api/ssh/groups/{id}", web::put().to(update_group))
            .route("/api/ssh/groups/{id}", web::delete().to(delete_group))
            .route("/api/ssh/servers", web::get().to(list_servers))
            .route("/api/ssh/servers", web::post().to(create_server))
            .route("/api/ssh/servers/{id}", web::put().to(update_server))
            .route("/api/ssh/servers/{id}", web::delete().to(delete_server))
            // SSH 实时监控 API
            .route("/api/ssh/monitor", web::post().to(ssh_monitor_api::get_monitor_stats))
            // 兼容预检请求
            .route("/api/ssh/monitor", web::route().guard(actix_web::guard::Options()).to(|| async { HttpResponse::NoContent().finish() }))
            // 使用模板引擎渲染 index.html
            .route("/", web::get().to(serve_index))
            .route("/index.html", web::get().to(serve_index))
            // Task API routes
            .route("/api/task/start", web::post().to(start_task))
            .route("/api/task/stop", web::post().to(stop_task))
            .route("/api/task/status", web::get().to(task_status))
            // API Test 定义 CRUD 路由
            .route("/api/apitest/endpoints", web::get().to(apitest_list))
            .route("/api/apitest/endpoints/{id}", web::get().to(apitest_get))
            .route("/api/apitest/endpoints", web::post().to(apitest_create))
            .route("/api/apitest/endpoints/{id}", web::put().to(apitest_update))
            .route(
                "/api/apitest/endpoints/{id}",
                web::delete().to(apitest_delete),
            )
            // 配置静态文件服务
            .service(
                actix_files::Files::new("/", &config_clone.static_files.root_dir)
                    .show_files_listing(),
            )
    })
    .workers(2)
    .bind(format!("{}:{}", config.server.host, config.server.port))?
    .run();
    Ok(server)
}

/// 使用轻量级模板引擎渲染 index.html，将 <rsp:include page="..." /> 嵌入对应文件内容
async fn serve_index(cfg: web::Data<crate::modules::config::config::Config>) -> impl Responder {
    let root = &cfg.static_files.root_dir;
    match render_with_includes(root, "index.html") {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(e) => HttpResponse::InternalServerError()
            .content_type("text/plain; charset=utf-8")
            .body(format!("template render error: {}", e)),
    }
}

// 运行Actix-Web服务器
pub async fn run() -> Result<(), std::io::Error> {
    // 读取配置文件
    let config = get_config().expect("Failed to load configuration");
    log_info!("🚀 启动web服务器...");
    log_info!(
        "📁 静态文件服务已启用,根目录{}",
        config.static_files.root_dir
    );
    let server = setup_actix().await?;
    server.await?;
    Ok(())
}

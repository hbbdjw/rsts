#[cfg(test)]
mod tests {
    use crate::modules::web::auth_middleware::AuthMiddleware;
    use crate::modules::web::auth_utils::{
        CODE_SUCCESS, CODE_TOKEN_EXPIRED, CODE_TOKEN_INVALID, Claims, JWT_SECRET, create_token,
        verify_token,
    };
    use crate::modules::web::database::Database;
    use crate::modules::web::login_handler;
    use crate::modules::web::models::{LoginRequest, Response};
    use actix_web::dev::Service; // Import Service trait
    use actix_web::{
        App, HttpMessage,
        http::{StatusCode, header},
        test, web,
    };
    use chrono::Duration;
    use std::sync::Arc;

    // Helper to setup DB
    fn setup_db() -> Arc<Database> {
        // Create in-memory DB which will be initialized with default users
        // admin / password123
        // user1 / user123
        let db = Database::new(":memory:").expect("Failed to create in-memory DB");
        Arc::new(db)
    }

    #[test]
    async fn test_token_utils() {
        let user_id = "1";
        let username = "testuser";

        // 1. Test Access Token Generation and Verification
        let token = create_token(user_id, username, "access", Duration::minutes(15)).unwrap();
        let claims = verify_token(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.username, username);
        assert_eq!(claims.token_type, "access");

        // 2. Test Expired Token
        // Create a token that expired 1 second ago
        let expired_token =
            create_token(user_id, username, "access", Duration::seconds(-1)).unwrap();
        let result = verify_token(&expired_token);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::ExpiredSignature
        ));
    }

    #[actix_web::test]
    async fn test_login_success() {
        let db = setup_db();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db.clone()))
                .route("/login", web::post().to(login_handler::login)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&LoginRequest {
                username: "admin".to_string(),
                password: "password123".to_string(),
            })
            .to_request();

        let resp: Response = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.code, CODE_SUCCESS);
        assert_eq!(resp.msg, "登录成功");
        assert!(resp.data.is_some());

        let data = resp.data.unwrap();
        assert!(data.get("token").is_some());
        assert!(data.get("refreshToken").is_some());
    }

    #[actix_web::test]
    async fn test_login_failure() {
        let db = setup_db();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db.clone()))
                .route("/login", web::post().to(login_handler::login)),
        )
        .await;

        // Wrong password
        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&LoginRequest {
                username: "admin".to_string(),
                password: "wrongpassword".to_string(),
            })
            .to_request();

        let resp: Response = test::call_and_read_body_json(&app, req).await;
        // Check code for invalid credentials (1001)
        assert_eq!(resp.code, "1001");

        // Non-existent user
        let req2 = test::TestRequest::post()
            .uri("/login")
            .set_json(&LoginRequest {
                username: "nonexistent".to_string(),
                password: "password".to_string(),
            })
            .to_request();

        let resp2: Response = test::call_and_read_body_json(&app, req2).await;
        // Check code for user not found (1002)
        assert_eq!(resp2.code, "1002");
    }

    #[actix_web::test]
    async fn test_auth_middleware() {
        let db = setup_db();

        // Initialize service with middleware and a protected route
        let app = test::init_service(
            App::new().app_data(web::Data::new(db.clone())).service(
                web::resource("/protected")
                    .wrap(AuthMiddleware)
                    .route(web::get().to(login_handler::get_user_info)),
            ),
        )
        .await;

        // 1. No Token
        let req = test::TestRequest::get().uri("/protected").to_request();
        // Use app.call instead of test::call_service to handle errors
        let resp = app.call(req).await;
        // Middleware returns Error for unauthorized, which Actix converts to 401
        match resp {
            Ok(_) => panic!("Expected unauthorized error"),
            Err(err) => {
                let res = err.error_response();
                assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
            }
        }

        // 2. Invalid Token
        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header((header::AUTHORIZATION, "Bearer invalidtoken"))
            .to_request();
        let resp = app.call(req).await;
        match resp {
            Ok(_) => panic!("Expected unauthorized error"),
            Err(err) => {
                let res = err.error_response();
                assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
            }
        }

        // 3. Expired Token
        let expired_token = create_token("1", "admin", "access", Duration::seconds(-10)).unwrap();
        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header((header::AUTHORIZATION, format!("Bearer {}", expired_token)))
            .to_request();
        let resp = app.call(req).await;
        match resp {
            Ok(_) => panic!("Expected unauthorized error"),
            Err(err) => {
                let res = err.error_response();
                assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
            }
        }

        // 4. Valid Token
        // First get valid token via login or create manually
        // We need user id for "admin" which is usually 1
        let valid_token = create_token("1", "admin", "access", Duration::minutes(15)).unwrap();

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header((header::AUTHORIZATION, format!("Bearer {}", valid_token)))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Check body content
        let body: Response = test::read_body_json(resp).await;
        assert_eq!(body.code, CODE_SUCCESS);
        // Verify user info is returned
        let data = body.data.unwrap();
        assert_eq!(data["userName"], "admin");
    }

    #[actix_web::test]
    async fn test_refresh_token() {
        let db = setup_db();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(db.clone()))
                .route("/refresh", web::post().to(login_handler::refresh_token)),
        )
        .await;

        // 1. Valid Refresh Token
        let refresh_token = create_token("1", "admin", "refresh", Duration::days(7)).unwrap();
        let req = test::TestRequest::post()
            .uri("/refresh")
            .set_json(serde_json::json!({ "refreshToken": refresh_token }))
            .to_request();

        let resp: Response = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.code, CODE_SUCCESS);
        let data = resp.data.unwrap();
        assert!(data.get("token").is_some());
        assert!(data.get("refreshToken").is_some());

        // 2. Invalid Refresh Token (Wrong Type)
        let access_token = create_token("1", "admin", "access", Duration::minutes(15)).unwrap();
        println!("Generated Access Token for Refresh Test: {}", access_token);

        let req = test::TestRequest::post()
            .uri("/refresh")
            .set_json(serde_json::json!({ "refreshToken": access_token }))
            .to_request();

        // Use call_service to get response, inspect status
        let resp = test::call_service(&app, req).await;
        println!("Response Status for Wrong Type: {}", resp.status());
        if resp.status() == StatusCode::OK {
            let body: Response = test::read_body_json(resp).await;
            println!("Unexpected OK Response: {:?}", body);
            panic!("Expected 401 but got 200 OK with body: {:?}", body);
        }
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // 3. Expired Refresh Token
        let expired_refresh =
            create_token("1", "admin", "refresh", Duration::seconds(-10)).unwrap();
        let req = test::TestRequest::post()
            .uri("/refresh")
            .set_json(serde_json::json!({ "refreshToken": expired_refresh }))
            .to_request();

        // Expired token decoding fails in handler, so handler returns Unauthorized response (not error)
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}

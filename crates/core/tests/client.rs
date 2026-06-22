//! Интеграционные тесты клиента против мок-сервера.

use std::time::Duration;

use fortochka_core::retry::Backoff;
use fortochka_core::{ApiClient, CoreError};
use fortochka_proto::{RegisterCameraResponse, routes};
use httpmock::prelude::*;

fn client(server: &MockServer) -> ApiClient {
    ApiClient::new(&server.base_url())
        .unwrap()
        .with_backoff(Backoff {
            attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        })
}

#[test]
fn register_camera_parses_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(routes::REGISTER_CAMERA);
        then.status(200).json_body_obj(&RegisterCameraResponse {
            camera_id: "cam-1".into(),
            upload_token: "secret".into(),
            pairing_code: "ABC123".into(),
        });
    });

    let resp = client(&server).register_camera("Дача").unwrap();
    assert_eq!(resp.camera_id, "cam-1");
    assert_eq!(resp.pairing_code, "ABC123");
}

#[test]
fn api_error_body_is_surfaced() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path(routes::PAIR);
        then.status(404).json_body(
            serde_json::json!({"code": "invalid_pairing_code", "message": "нет такого кода"}),
        );
    });

    let err = client(&server).pair("WRONG1").unwrap_err();
    match err {
        CoreError::Api { status, message } => {
            assert_eq!(status, 404);
            assert_eq!(message, "нет такого кода");
        }
        other => panic!("ожидали Api, получили: {other:?}"),
    }
}

#[test]
fn client_errors_are_not_retried() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path(routes::PAIR);
        then.status(404);
    });

    let _ = client(&server).pair("WRONG1").unwrap_err();
    mock.assert_hits(1);
}

#[test]
fn server_errors_are_retried_then_given_up() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path(routes::HEALTHZ);
        then.status(500);
    });

    let err = client(&server).health().unwrap_err();
    assert!(err.is_transient());
    mock.assert_hits(3);
}

#[test]
fn wallpaper_returns_raw_bytes() {
    let server = MockServer::start();
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
    server.mock(|when, then| {
        when.method(GET)
            .path(routes::wallpaper("tok"))
            .query_param("w", "1080")
            .query_param("h", "2400");
        then.status(200)
            .header("content-type", "image/jpeg")
            .body(&jpeg);
    });

    let bytes = client(&server).fetch_wallpaper("tok", 1080, 2400).unwrap();
    assert_eq!(bytes, jpeg);
}

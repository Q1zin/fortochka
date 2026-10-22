//! Интеграционные тесты полного цикла: настоящий сервер на случайном порту,
//! настоящий клиент из fortochka-core. Проверяется не только сервер,
//! но и совместимость клиента с ним на уровне провода.

use std::sync::mpsc;
use std::time::Duration;

use fortochka_core::retry::Backoff;
use fortochka_core::{ApiClient, CoreError};
use image::GenericImageView;

struct TestServer {
    base_url: String,
    // Держим tempdir живым, пока жив сервер.
    _data_dir: tempfile::TempDir,
}

fn spawn_server() -> TestServer {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dir = data_dir.path().to_owned();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async move {
            let state = fortochka_server::AppState::init(&dir).await.expect("init");
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            tx.send(listener.local_addr().expect("addr")).expect("send");
            axum::serve(listener, fortochka_server::app(state))
                .await
                .expect("serve");
        });
    });

    let addr = rx.recv().expect("сервер не поднялся");
    TestServer {
        base_url: format!("http://{addr}"),
        _data_dir: data_dir,
    }
}

fn client(server: &TestServer) -> ApiClient {
    ApiClient::new(&server.base_url)
        .expect("client")
        .with_backoff(Backoff {
            attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        })
}

#[allow(clippy::cast_possible_truncation)] // x % 256 всегда влезает в u8
fn test_jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 200])
    });
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80)
        .encode_image(&image::DynamicImage::ImageRgb8(img))
        .unwrap();
    buf
}

#[test]
fn full_cycle_register_upload_pair_view() {
    let server = spawn_server();
    let api = client(&server);

    api.health().expect("healthz");

    let camera = api.register_camera("Дача").expect("register");
    assert_eq!(camera.pairing_code.len(), 6);

    api.upload_frame(
        &camera.camera_id,
        &camera.upload_token,
        &test_jpeg(400, 800),
    )
    .expect("upload");

    let paired = api.pair(&camera.pairing_code).expect("pair");
    assert_eq!(paired.camera_name, "Дача");

    let wallpaper = api
        .fetch_wallpaper(&paired.view_token, 108, 240)
        .expect("wallpaper");
    let img = image::load_from_memory(&wallpaper).expect("валидный JPEG");
    assert_eq!(img.dimensions(), (108, 240));
}

#[test]
fn view_status_reports_last_frame_time() {
    let server = spawn_server();
    let api = client(&server);
    let camera = api.register_camera("Статус").expect("register");
    let paired = api.pair(&camera.pairing_code).expect("pair");

    let before = api.view_status(&paired.view_token).expect("статус");
    assert_eq!(before.camera_name, "Статус");
    assert_eq!(before.last_frame_at, None);

    let sent = api
        .upload_frame(&camera.camera_id, &camera.upload_token, &test_jpeg(40, 80))
        .expect("upload");
    let after = api.view_status(&paired.view_token).expect("статус");
    assert_eq!(after.last_frame_at, Some(sent.received_at));

    let err = api.view_status("чужой-токен").unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 404, .. }), "{err:?}");
}

#[test]
fn deleted_camera_disappears_for_viewers() {
    let server = spawn_server();
    let api = client(&server);
    let camera = api.register_camera("Удаляемая").expect("register");
    api.upload_frame(&camera.camera_id, &camera.upload_token, &test_jpeg(40, 80))
        .expect("upload");
    let paired = api.pair(&camera.pairing_code).expect("pair");

    api.delete_camera(&camera.camera_id, &camera.upload_token)
        .expect("delete");

    let err = api.fetch_wallpaper(&paired.view_token, 10, 20).unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 404, .. }), "{err:?}");
    let err = api.pair(&camera.pairing_code).unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 404, .. }), "{err:?}");
}

#[test]
fn delete_requires_upload_token() {
    let server = spawn_server();
    let api = client(&server);
    let camera = api.register_camera("Чужая").expect("register");

    let err = api
        .delete_camera(&camera.camera_id, "не-тот-токен")
        .unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 401, .. }), "{err:?}");
    api.pair(&camera.pairing_code).expect("камера на месте");
}

#[test]
fn pairing_code_is_case_insensitive() {
    let server = spawn_server();
    let api = client(&server);

    let camera = api.register_camera("Аквариум").expect("register");
    let lower = camera.pairing_code.to_lowercase();
    api.pair(&lower).expect("пейринг в нижнем регистре");
}

#[test]
fn camera_name_limit_counts_characters_not_bytes() {
    let server = spawn_server();
    let api = client(&server);

    // 100 кириллических букв — это 200 байт, но всего 100 символов
    api.register_camera(&"Д".repeat(100))
        .expect("100 символов — в пределах лимита");
    let err = api.register_camera(&"Д".repeat(101)).unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 400, .. }), "{err:?}");
}

#[test]
fn wrong_upload_token_is_unauthorized() {
    let server = spawn_server();
    let api = client(&server);

    let camera = api.register_camera("Офис").expect("register");
    let err = api
        .upload_frame(&camera.camera_id, "чужой-токен", &test_jpeg(4, 4))
        .unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 401, .. }), "{err:?}");
}

#[test]
fn garbage_body_is_rejected() {
    let server = spawn_server();
    let api = client(&server);

    let camera = api.register_camera("Тест").expect("register");
    let err = api
        .upload_frame(&camera.camera_id, &camera.upload_token, b"not a jpeg")
        .unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 400, .. }), "{err:?}");
}

#[test]
fn unknown_pairing_code_is_not_found() {
    let server = spawn_server();
    let err = client(&server).pair("НЕТТАК").unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 404, .. }), "{err:?}");
}

#[test]
fn wallpaper_before_first_frame_is_not_found() {
    let server = spawn_server();
    let api = client(&server);

    let camera = api.register_camera("Пустая").expect("register");
    let paired = api.pair(&camera.pairing_code).expect("pair");
    let err = api
        .fetch_wallpaper(&paired.view_token, 100, 200)
        .unwrap_err();
    assert!(matches!(err, CoreError::Api { status: 404, .. }), "{err:?}");
}

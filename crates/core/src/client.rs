use std::time::Duration;

use fortochka_proto::{
    ApiErrorBody, FrameUploadResponse, PairRequest, PairResponse, RegisterCameraRequest,
    RegisterCameraResponse, routes,
};
use reqwest::blocking::{Client, Response};
use url::Url;

use crate::error::CoreError;
use crate::retry::{Backoff, retry};

/// Синхронный клиент API сервера.
pub struct ApiClient {
    base: Url,
    http: Client,
    backoff: Backoff,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Result<Self, CoreError> {
        let base = Url::parse(base_url)
            .map_err(|e| CoreError::InvalidBaseUrl(format!("{base_url}: {e}")))?;
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_mins(1))
            .user_agent(concat!("fortochka-core/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            base,
            http,
            backoff: Backoff::default(),
        })
    }

    /// Заменить политику повторов (в тестах — с нулевыми задержками).
    #[must_use]
    pub fn with_backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = backoff;
        self
    }

    pub fn health(&self) -> Result<(), CoreError> {
        let url = self.url(routes::HEALTHZ)?;
        self.with_retry(|| {
            let resp = self.http.get(url.clone()).send()?;
            Self::expect_success(resp).map(drop)
        })
    }

    pub fn register_camera(&self, name: &str) -> Result<RegisterCameraResponse, CoreError> {
        let url = self.url(routes::REGISTER_CAMERA)?;
        let body = RegisterCameraRequest {
            name: name.to_owned(),
        };
        self.with_retry(|| {
            let resp = self.http.post(url.clone()).json(&body).send()?;
            Ok(Self::expect_success(resp)?.json()?)
        })
    }

    /// Загрузка кадра идемпотентна (сервер хранит только последний),
    /// поэтому повторять её безопасно.
    pub fn upload_frame(
        &self,
        camera_id: &str,
        upload_token: &str,
        jpeg: &[u8],
    ) -> Result<FrameUploadResponse, CoreError> {
        let url = self.url(&routes::camera_frame(camera_id))?;
        self.with_retry(|| {
            let resp = self
                .http
                .post(url.clone())
                .bearer_auth(upload_token)
                .header(reqwest::header::CONTENT_TYPE, "image/jpeg")
                .body(jpeg.to_vec())
                .send()?;
            Ok(Self::expect_success(resp)?.json()?)
        })
    }

    pub fn pair(&self, pairing_code: &str) -> Result<PairResponse, CoreError> {
        let url = self.url(routes::PAIR)?;
        let body = PairRequest {
            pairing_code: pairing_code.to_owned(),
        };
        self.with_retry(|| {
            let resp = self.http.post(url.clone()).json(&body).send()?;
            Ok(Self::expect_success(resp)?.json()?)
        })
    }

    /// Свежий кадр, уже кропнутый сервером под экран `width`×`height`.
    pub fn fetch_wallpaper(
        &self,
        view_token: &str,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, CoreError> {
        let mut url = self.url(&routes::wallpaper(view_token))?;
        url.query_pairs_mut()
            .append_pair("w", &width.to_string())
            .append_pair("h", &height.to_string());
        self.with_retry(|| {
            let resp = self.http.get(url.clone()).send()?;
            Ok(Self::expect_success(resp)?.bytes()?.to_vec())
        })
    }

    fn url(&self, path: &str) -> Result<Url, CoreError> {
        self.base
            .join(path)
            .map_err(|e| CoreError::InvalidBaseUrl(e.to_string()))
    }

    fn with_retry<T>(&self, op: impl FnMut() -> Result<T, CoreError>) -> Result<T, CoreError> {
        retry(&self.backoff, CoreError::is_transient, op)
    }

    /// 2xx → ответ как есть; иначе — `CoreError::Api`, по возможности
    /// с сообщением из тела ошибки сервера.
    fn expect_success(resp: Response) -> Result<Response, CoreError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let message = resp.json::<ApiErrorBody>().map_or_else(
            |_| {
                status
                    .canonical_reason()
                    .unwrap_or("неизвестная ошибка")
                    .to_owned()
            },
            |body| body.message,
        );
        Err(CoreError::Api {
            status: status.as_u16(),
            message,
        })
    }
}

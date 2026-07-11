use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client,
};

/// Construit un `reqwest::Client` partagé avec des en-têtes par défaut
/// (Bearer / x-api-key positionnés une fois pour toutes) et un timeout.
pub fn build_client(extra_headers: &[(&str, &str)]) -> reqwest::Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    for (k, v) in extra_headers {
        headers.insert(
            HeaderName::from_bytes(k.as_bytes()).expect("invalid header name"),
            HeaderValue::from_str(v).expect("invalid header value"),
        );
    }
    Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(120))
        .build()
}

//! Helpers to provide mock HTTP responses for test fixtures.

use anyhow::anyhow;
use bytes::Bytes;
use http::header::HOST;
use serde::Deserialize;
use serde_json::Value;

/// Configuration for mocking fetch requests.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Fetch {
    /// Authority (host) to match for mock fetch requests.
    pub authority: Option<String>,

    /// Method to match for mock fetch requests.
    ///
    /// Defaults to GET.
    #[serde(default)]
    pub method: Method,

    /// Path to match for mock fetch requests, not including query parameters.
    #[serde(default = "default_path")]
    pub path: String,

    /// String to uniquely identify a fetch request.
    ///
    /// This simulates a query string or body content to differentiate requests
    /// so a serialized representation of those could be used in test fixtures,
    /// or some abbreviated identifier.
    pub request: Option<String>,

    /// Expected response if all the other fields match.
    #[serde(default)]
    pub response: Response,
}

fn default_path() -> String {
    "/".to_string()
}

/// Supported HTTP verbs (methods) for fetch requests.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    /// GET method.
    #[default]
    Get,
    /// POST method.
    Post,
    /// PUT method.
    Put,
    /// DELETE method.
    Delete,
    /// PATCH method.
    Patch,
}

/// Mock HTTP response for fetch requests.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Response {
    /// HTTP status code.
    ///
    /// Defaults to 200 so can be omitted in test fixtures unless a specific
    /// status is asserted.
    pub status: u16,

    /// Response body.
    ///
    /// This is a `Value` that the test is expected to deserialize as needed.
    /// Defaults to an empty string for tests that do not require asserting on
    /// response body contents.
    pub body: Value,
}

impl Default for Response {
    fn default() -> Self {
        Self { status: 200, body: Value::String(String::new()) }
    }
}

/// Collection of fetch request configurations that can be used in an
/// `HttpRequest` capability.
#[derive(Clone, Debug, Deserialize)]
pub struct Fetcher {
    /// List of fetch request configurations.
    pub fetches: Vec<Fetch>,
}

impl Fetcher {
    /// Create a new Fetcher with the given fetch request configurations.
    #[must_use]
    pub fn new(fetches: &[Fetch]) -> Self {
        Self { fetches: fetches.to_vec() }
    }

    /// Simulate fetching a request by finding a matching fetch configuration
    /// and returning the response.
    ///
    /// # Errors
    ///
    /// Returns an error when the request method is unsupported, the authority
    /// or host header is missing, or no matching fetch configuration is found.
    pub fn fetch<T>(&self, request: &http::Request<T>) -> anyhow::Result<http::Response<Bytes>> {
        let method = match *request.method() {
            http::Method::GET => Method::Get,
            http::Method::POST => Method::Post,
            http::Method::PUT => Method::Put,
            http::Method::DELETE => Method::Delete,
            http::Method::PATCH => Method::Patch,
            _ => return Err(anyhow!("unsupported HTTP method: {}", request.method())),
        };

        let authority = request
            .uri()
            .authority()
            .map(|auth| auth.as_str().to_owned())
            .or_else(|| {
                request.headers().get(HOST).and_then(|value| value.to_str().ok().map(str::to_owned))
            })
            .ok_or_else(|| anyhow!("request missing authority or host header"))?;

        let path = request.uri().path().to_owned();
        let request_id = request.uri().query().map(str::to_owned);

        let fetch = self.fetches.iter().find(|candidate| {
            candidate.authority.as_ref().is_none_or(|auth| auth == &authority)
                && candidate.method == method
                && candidate.path == path
                && (candidate.request.is_none() || candidate.request == request_id)
        });

        let fetch = fetch.ok_or_else(|| {
            anyhow!(
                "no fetch configured for method={method:?}, authority={authority}, path={path}, request={request_id:?}"
            )
        })?;

        let status = fetch.response.status;
        let body = Bytes::from(fetch.response.body.to_string());

        http::Response::builder().status(status).body(body).map_err(anyhow::Error::new)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn build_request(method: http::Method, uri: &str, host: Option<&str>) -> http::Request<()> {
        let mut builder = http::Request::builder().method(method).uri(uri);
        if let Some(host_value) = host {
            builder = builder.header(HOST, host_value);
        }
        builder.body(()).expect("should build request")
    }

    #[test]
    fn fetch_deserialize() {
        let json_data = r#"
        {
            "authority": "example.com",
            "method": "GET",
            "path": "/api/data",
            "response": {
                "status": 404,
                "body": "Not Found"
            }
        }
        "#;

        let fetch: Fetch = serde_json::from_str(json_data).expect("should deserialize Fetch");
        assert_eq!(fetch.authority, Some("example.com".to_string()));
        assert_eq!(fetch.method, Method::Get);
        assert_eq!(fetch.path, "/api/data");
        assert_eq!(fetch.response.status, 404);
        assert_eq!(fetch.response.body, "Not Found");
    }

    #[test]
    fn fetch_deserialize_default() {
        let json_data = "{}";
        let fetch: Fetch = serde_json::from_str(json_data).expect("should deserialize Fetch");
        assert_eq!(fetch.authority, None);
        assert_eq!(fetch.method, Method::Get);
        assert_eq!(fetch.path, "/");
        assert_eq!(fetch.response.status, 200);
        assert_eq!(fetch.response.body, "");
    }

    #[test]
    fn fetch_partial_deserialize() {
        let json_data = r#"
            {
                "path": "/allocations/trips",
                "response": {
                    "body": "[\"vehicle 1\"]"
                }
            }
        "#;

        let fetch: Fetch = serde_json::from_str(json_data).expect("should deserialize Fetch");
        assert_eq!(fetch.authority, None);
        assert_eq!(fetch.method, Method::Get);
        assert_eq!(fetch.path, "/allocations/trips");
        assert_eq!(fetch.response.status, 200);
        assert_eq!(fetch.response.body, "[\"vehicle 1\"]");
    }

    #[test]
    fn fetcher_matches_authority() {
        let fetch = Fetch {
            authority: Some("api.example.com".to_string()),
            method: Method::Get,
            path: "/data".to_string(),
            request: Some("q=42".to_string()),
            response: Response { status: 201, body: json!({"value": 42}) },
        };
        let fetcher = Fetcher::new(&[fetch]);

        let request = build_request(http::Method::GET, "https://api.example.com/data?q=42", None);
        let response = fetcher.fetch(&request).expect("should find mock fetch");

        assert_eq!(response.status(), 201);
        assert_eq!(response.body(), &Bytes::from("{\"value\":42}".to_string()));
    }

    #[test]
    fn fetcher_matches_host_header() {
        let fetch = Fetch {
            authority: Some("example.com".to_string()),
            method: Method::Get,
            path: "/allocations".to_string(),
            request: Some("vehicle=1".to_string()),
            response: Response { status: 200, body: json!([1]) },
        };
        let fetcher = Fetcher::new(&[fetch]);

        let request =
            build_request(http::Method::GET, "/allocations?vehicle=1", Some("example.com"));
        let response = fetcher.fetch(&request).expect("should match host header");

        assert_eq!(response.status(), 200);
        assert_eq!(response.body(), &Bytes::from("[1]"));
    }

    #[test]
    fn fetcher_missing_entry_errors() {
        let fetcher = Fetcher::new(&[]);
        let request = build_request(http::Method::GET, "https://example.com/api", None);

        let error = fetcher.fetch(&request).expect_err("should fail without mock");
        assert!(error.to_string().contains("no fetch configured"));
    }
}

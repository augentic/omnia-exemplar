//! End-to-end smoke test for the assembled guest and example host.
//!
//! Spawns `examples/runtime.rs` (the native host) with the release
//! `guest.wasm`, waits for its HTTP listener, drives one request through
//! every route, and finally requires the host log to show a messaging
//! delivery. Starting the host is itself a check: `omnia::runtime!`
//! pre-instantiates the component, so a Provider/host drift fails before
//! the port ever opens.
//!
//! The assertions check **dispatch, not semantics**: a `404`/`405` means a
//! route is miswired; any other status means the request reached its
//! handler. Full success is asserted only where the in-tree default
//! backends suffice. Business logic is covered by each crate's own tests.
//!
//! IMPORTANT: Assumes the guest and the host are already built; run via
//! `cargo make smoke`, which builds both first. The test is `#[ignore]`d so
//! the regular `cargo make test` loop skips it. It uses only `std` so the
//! dependency tree (and `cargo vet`) is unchanged.

#![cfg(not(target_arch = "wasm32"))]

use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::fs::{self, File};
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

/// Where the host's `WasiHttp` listener answers.
const HTTP_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080));
/// A closed loopback port: every upstream call is refused immediately.
const CLOSED_UPSTREAM: &str = "http://127.0.0.1:9";
/// A debug host compiles the component before it listens.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Emitted by `omnia_wasi_messaging` when it delivers a message to the guest.
const LOG_NEEDLE: &str = "message_counter";
const LOG_NEEDLE_TIMEOUT: Duration = Duration::from_secs(15);

/// Host environment: real keys, dummy values. Upstreams point at a closed
/// port so handlers that call out fail fast; `IdentityDefault` only stores
/// its credentials at connect time, so placeholders are enough to start.
const HOST_ENV: &[(&str, &str)] = &[
    ("ENV", "dev"),
    ("BLOCK_MGT_URL", CLOSED_UPSTREAM),
    ("FLEET_URL", CLOSED_UPSTREAM),
    ("TRIP_MANAGEMENT_URL", CLOSED_UPSTREAM),
    ("STATIC_API_URL", CLOSED_UPSTREAM),
    ("PATTERN_DECODER_URL", CLOSED_UPSTREAM),
    ("API_IDENTITY", "smoke"),
    ("IDENTITY_CLIENT_ID", "smoke"),
    ("IDENTITY_CLIENT_SECRET", "smoke"),
    ("IDENTITY_TOKEN_URL", "http://127.0.0.1:9/token"),
    ("PATTERN_CLIENT_CERT", "smoke"),
    ("GOD_MODE_ENABLED", "true"),
    // Ephemeral port: the default `0.0.0.0:80` fails to bind unprivileged.
    ("WEBSOCKET_ADDR", "127.0.0.1:0"),
];

const TALLY_MESSAGE: &[u8] = include_bytes!("../crates/tally-connector/data/tally-message.json");
const RECEIVE_MESSAGE: &[u8] = include_bytes!("../crates/pulse-connector/data/receive-message.xml");

/// What a check requires of the response status.
#[derive(Clone, Copy)]
enum Expect {
    /// Exactly this status.
    Status(u16),
    /// Any 2xx: the default backend serves the route in full.
    Success,
    /// Anything but 404/405: the route is wired and the handler ran.
    Dispatched,
    /// Anything but 400/404/405: the route is wired and its codec decoded.
    Decoded,
}

impl Expect {
    const fn accepts(self, status: u16) -> bool {
        match self {
            Self::Status(want) => status == want,
            Self::Success => matches!(status, 200..=299),
            Self::Dispatched => !matches!(status, 404 | 405),
            Self::Decoded => !matches!(status, 400 | 404 | 405),
        }
    }
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(want) => write!(f, "want {want}"),
            Self::Success => f.write_str("want 2xx"),
            Self::Dispatched => f.write_str("want anything but 404/405"),
            Self::Decoded => f.write_str("want anything but 400/404/405"),
        }
    }
}

/// One request and what its response must satisfy.
struct Check {
    label: &'static str,
    method: &'static str,
    path: &'static str,
    /// `(content-type, bytes)`.
    body: Option<(&'static str, &'static [u8])>,
    expect: Expect,
    /// Required `Content-Type` prefix of the response.
    content_type: Option<&'static str>,
    /// Required substring of the response body.
    body_contains: Option<&'static str>,
}

impl Check {
    const fn new(
        method: &'static str, label: &'static str, path: &'static str, expect: Expect,
    ) -> Self {
        Self {
            label,
            method,
            path,
            body: None,
            expect,
            content_type: None,
            body_contains: None,
        }
    }

    const fn get(label: &'static str, path: &'static str, expect: Expect) -> Self {
        Self::new("GET", label, path, expect)
    }

    const fn post(label: &'static str, path: &'static str, expect: Expect) -> Self {
        Self::new("POST", label, path, expect)
    }

    const fn json(mut self, body: &'static [u8]) -> Self {
        self.body = Some(("application/json", body));
        self
    }

    const fn xml(mut self, body: &'static [u8]) -> Self {
        self.body = Some(("text/xml", body));
        self
    }

    const fn content_type(mut self, prefix: &'static str) -> Self {
        self.content_type = Some(prefix);
        self
    }

    const fn body_contains(mut self, needle: &'static str) -> Self {
        self.body_contains = Some(needle);
        self
    }
}

/// Every route in `src/lib.rs`, in router order. Checks run sequentially
/// against one host, so later checks may rely on state written by earlier
/// ones (the docstore and SQL creates feed the reads that follow).
const CHECKS: &[Check] = &[
    Check::get("baseline/unknown-route", "/nope", Expect::Status(404)),
    Check::post("apc/tally-message", "/api/apc", Expect::Status(200)).json(TALLY_MESSAGE),
    // The README's codec contract: malformed XML is answered with the
    // vendor's fault, not the framework's plain-text 400.
    Check::post("pulse/garbage-is-a-fault", "/inbound/xml", Expect::Status(400))
        .xml(b"<garbage/>")
        .content_type("text/xml")
        .body_contains("<Fault>"),
    Check::post("pulse/receive-message", "/inbound/xml", Expect::Status(200))
        .xml(RECEIVE_MESSAGE)
        .content_type("text/xml"),
    // Calls upstream APIs that are not there: dispatch only.
    Check::get("gtfs/vehicle-info", "/info/123", Expect::Dispatched),
    // Present only when the guest is built with `--all-features`.
    Check::post("god-mode/set-trip", "/god-mode/set-trip/v1/t1", Expect::Dispatched).json(b"{}"),
    // Docstore examples against the in-memory `DocStoreDefault`.
    Check::post("docstore/create-stop", "/examples/stops", Expect::Success).json(
        br#"{"id":"smoke-stop","stop_name":"Smoke Test Stop","stop_lat":-36.8442,"stop_lon":174.7676,"zone_id":"zone-1"}"#,
    ),
    Check::get("docstore/get-stop", "/examples/stops/smoke-stop", Expect::Status(200)),
    Check::get("docstore/list-stops", "/examples/stops", Expect::Status(200)),
    Check::get("docstore/list-routes", "/examples/routes", Expect::Status(200)),
    Check::get("docstore/list-stop-times", "/examples/stop-times", Expect::Status(200)),
    Check::new("DELETE", "docstore/delete-stop", "/examples/stops/smoke-stop", Expect::Success),
    // SQL examples: `schema::ensure` creates the tables against `SqlDefault`.
    Check::post("sql/create-agency", "/examples/agencies", Expect::Success).json(
        br#"{"name":"Smoke Transit","url":"https://smoke.example","timezone":"Pacific/Auckland"}"#,
    ),
    Check::get("sql/list-agencies", "/examples/agencies", Expect::Status(200)),
    Check::get("sql/list-feeds", "/examples/feeds", Expect::Status(200)),
    // Pattern examples: prove the codecs decode, not the backends.
    Check::post("patterns/upsert-place", "/examples/patterns/places", Expect::Decoded)
        .json(br#"{"id":"smoke-place","name":"Smoke Place","lat":-36.8442,"lon":174.7676}"#),
    // A GET whose body is decoded by the exemplar's own `handle_with` codec.
    Check::get("patterns/nearby-with-body", "/examples/patterns/nearby", Expect::Decoded)
        .json(br#"{"lat":-36.8442,"lon":174.7676,"radius_m":500}"#),
    Check::post("patterns/decode", "/examples/patterns/decode", Expect::Dispatched)
        .json(br#"{"code":"smoke"}"#),
    // Capability examples: blobstore and docstore defaults are in-memory.
    Check::post("capability/archive", "/examples/archive", Expect::Status(200))
        .json(br#"{"container":"smoke","name":"hello.txt","payload":"hello"}"#),
    Check::post("capability/note", "/examples/note", Expect::Status(200))
        .json(br#"{"store":"notes","id":"smoke-note","body":{"text":"hello"}}"#),
    Check::post("capability/alert", "/examples/alert", Expect::Dispatched)
        .json(br#"{"channel":"ops","message":"hello"}"#),
    // `ReadingRequest` writes to a table nothing creates: dispatch only.
    Check::post("capability/reading", "/examples/reading", Expect::Dispatched)
        .json(br#"{"connection":"db","sensor":"smoke","value":1.5}"#),
];

enum Outcome {
    Pass(String),
    Fail(String),
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass(message) => write!(f, "PASS {message}"),
            Self::Fail(message) => write!(f, "FAIL {message}"),
        }
    }
}

#[test]
#[ignore = "needs a built guest and host; run via cargo make smoke"]
fn assembled_guest_serves_every_route() -> Result<()> {
    let artifacts = Artifacts::locate()?;
    if port_open() {
        bail!("port {HTTP_ADDR} is already in use; stop that server first");
    }
    let log_path = std::env::temp_dir().join(format!("guest-smoke-{}.log", std::process::id()));
    println!("host log: {}", log_path.display());

    let mut host = Host::spawn(&artifacts, &log_path)?;
    host.wait_for_port()?;

    let mut outcomes = Vec::with_capacity(CHECKS.len() + 1);
    for check in CHECKS {
        let outcome = run_check(check);
        println!("{outcome}");
        outcomes.push(outcome);
    }
    let outcome = await_log_needle(&log_path);
    println!("{outcome}");
    outcomes.push(outcome);
    drop(host);

    let failed = outcomes.iter().filter(|outcome| matches!(outcome, Outcome::Fail(_))).count();
    println!();
    println!("===== SUMMARY =====");
    println!("pass: {}", outcomes.len() - failed);
    println!("fail: {failed}");
    for outcome in &outcomes {
        if matches!(outcome, Outcome::Fail(_)) {
            println!("{outcome}");
        }
    }
    println!("host log: {}", log_path.display());
    if failed > 0 {
        bail!("{failed} smoke check(s) failed; see the host log at {}", log_path.display());
    }
    Ok(())
}

/// The pre-built host binary and guest component.
struct Artifacts {
    host: PathBuf,
    wasm: PathBuf,
}

impl Artifacts {
    fn locate() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target =
            std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
        let host = target.join(format!("debug/examples/runtime{}", std::env::consts::EXE_SUFFIX));
        let wasm = target.join("wasm32-wasip2/release/guest.wasm");
        for (what, path) in [("host binary", &host), ("guest component", &wasm)] {
            if !path.is_file() {
                bail!(
                    "{what} not found at {}; run `cargo make smoke`, which builds it first",
                    path.display()
                );
            }
        }
        Ok(Self { host, wasm })
    }
}

/// The running host process; killed on drop so no failure path leaks it.
struct Host {
    child: Child,
}

impl Host {
    fn spawn(artifacts: &Artifacts, log_path: &Path) -> Result<Self> {
        let log = File::create(log_path)
            .with_context(|| format!("creating host log {}", log_path.display()))?;
        let rust_log = std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "info,omnia_wasi_messaging=debug".to_owned());
        let child = Command::new(&artifacts.host)
            .arg("run")
            .arg(&artifacts.wasm)
            .env("RUST_LOG", rust_log)
            .envs(HOST_ENV.iter().copied())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| format!("spawning {}", artifacts.host.display()))?;
        Ok(Self { child })
    }

    /// Wait for the HTTP listener; the host only listens once the component
    /// has been compiled and pre-instantiated, so this is the link proof.
    fn wait_for_port(&mut self) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            if port_open() {
                return Ok(());
            }
            if let Some(status) = self.child.try_wait()? {
                bail!("host exited during startup ({status})");
            }
            sleep(Duration::from_millis(500));
        }
        bail!("host did not open {HTTP_ADDR} within {STARTUP_TIMEOUT:?}")
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn run_check(check: &Check) -> Outcome {
    let label = check.label;
    let response = match request(check) {
        Ok(response) => response,
        Err(error) => return Outcome::Fail(format!("{label} (request failed: {error:#})")),
    };
    let mut problems = Vec::new();
    if !check.expect.accepts(response.status) {
        problems.push(format!("status {} ({})", response.status, check.expect));
    }
    if let Some(want) = check.content_type
        && !response.header("content-type").is_some_and(|got| got.starts_with(want))
    {
        problems.push(format!("content-type {:?} (want {want})", response.header("content-type")));
    }
    if let Some(needle) = check.body_contains
        && !response.text().contains(needle)
    {
        problems.push(format!("body lacks {needle:?}"));
    }
    if problems.is_empty() {
        Outcome::Pass(format!("{label} ({})", response.status))
    } else {
        Outcome::Fail(format!(
            "{label}: {} body={}",
            problems.join(", "),
            snippet(&response.text())
        ))
    }
}

/// Once the checks have run, the `POST /inbound/xml` publish must have been
/// delivered to the guest's messaging export through the in-memory broker.
fn await_log_needle(log_path: &Path) -> Outcome {
    let deadline = Instant::now() + LOG_NEEDLE_TIMEOUT;
    loop {
        let log = fs::read(log_path).unwrap_or_default();
        if String::from_utf8_lossy(&log).contains(LOG_NEEDLE) {
            return Outcome::Pass(format!("messaging/delivery ({LOG_NEEDLE:?} in host log)"));
        }
        if Instant::now() >= deadline {
            return Outcome::Fail(format!(
                "messaging/delivery ({LOG_NEEDLE:?} not in host log within {LOG_NEEDLE_TIMEOUT:?})"
            ));
        }
        sleep(Duration::from_millis(500));
    }
}

/// A parsed HTTP/1.1 response.
struct Response {
    status: u16,
    /// Header names lower-cased.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn parse(raw: &[u8]) -> Result<Self> {
        let split = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .context("response has no end of headers")?;
        let head = std::str::from_utf8(&raw[..split]).context("response head is not UTF-8")?;
        let mut lines = head.split("\r\n");
        let status_line = lines.next().context("empty response")?;
        let status = status_line
            .split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .with_context(|| format!("malformed status line {status_line:?}"))?;
        let headers: Vec<(String, String)> = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
            .collect();
        let mut response = Self {
            status,
            headers,
            body: raw[split + 4..].to_vec(),
        };
        if response.header("transfer-encoding").is_some_and(|te| te.eq_ignore_ascii_case("chunked"))
        {
            response.body = dechunk(&response.body)?;
        } else if let Some(length) = response.header("content-length").and_then(|v| v.parse().ok())
        {
            response.body.truncate(length);
        }
        Ok(response)
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }
}

/// Send one request over a fresh connection and read the whole response.
///
/// `Connection: close` makes EOF the end of the response. A server that
/// keeps the socket open anyway surfaces as a read timeout, and whatever
/// arrived by then is parsed as the response.
fn request(check: &Check) -> Result<Response> {
    let mut stream = TcpStream::connect_timeout(&HTTP_ADDR, CONNECT_TIMEOUT)
        .with_context(|| format!("connecting to {HTTP_ADDR}"))?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;

    let body = check.body.map_or(&[][..], |(_, bytes)| bytes);
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: localhost:8080\r\nConnection: close\r\n",
        check.method, check.path
    );
    if let Some((content_type, _)) = check.body {
        write!(head, "Content-Type: {content_type}\r\n")?;
    }
    write!(head, "Content-Length: {}\r\n\r\n", body.len())?;
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;

    let mut raw = Vec::new();
    if let Err(error) = stream.read_to_end(&mut raw)
        && !matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
    {
        return Err(error).context("reading response");
    }
    Response::parse(&raw)
}

/// Decode a `Transfer-Encoding: chunked` body.
fn dechunk(mut rest: &[u8]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = rest
            .windows(2)
            .position(|window| window == b"\r\n")
            .context("truncated chunk-size line")?;
        let size_line =
            std::str::from_utf8(&rest[..line_end]).context("chunk size is not UTF-8")?;
        let size_hex = size_line.split_once(';').map_or(size_line, |(size, _)| size).trim();
        let size = usize::from_str_radix(size_hex, 16)
            .with_context(|| format!("malformed chunk size {size_line:?}"))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            return Ok(body);
        }
        body.extend_from_slice(rest.get(..size).context("truncated chunk")?);
        rest = rest.get(size + 2..).context("truncated chunk terminator")?;
    }
}

fn port_open() -> bool {
    TcpStream::connect_timeout(&HTTP_ADDR, Duration::from_millis(250)).is_ok()
}

/// First 200 characters of a body, for failure diagnostics.
fn snippet(text: &str) -> String {
    text.chars().take(200).collect()
}

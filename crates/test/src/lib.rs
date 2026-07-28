//! Fixture definitions and HTTP-replay helpers for adapter tests.

mod fetch;
mod testdef;

pub use fetch::{Fetch, Fetcher};
pub use testdef::{TestDef, TestResult};

/// Construct an input/output pair from a serialized test definition.
pub trait Fixture {
    /// Request type presented to the handler under test.
    type Input: Default;

    /// Expected success type observed by the test (handler return value,
    /// published message, etc.).
    type Output;

    /// Error type for failure cases.
    type Error: std::error::Error;

    /// Optional parameters that transform the raw input before invoke.
    type TransformParams;

    /// Build this fixture from a deserialized test definition.
    fn from_data(data_def: &TestDef<Self::Error>) -> Self;

    /// Input presented to the handler, if any.
    fn input(&self) -> Option<Self::Input>;

    /// Transform parameters, if any.
    fn params(&self) -> Option<Self::TransformParams> {
        None
    }

    /// Apply `f` to the input (or return `Input::default` when absent).
    fn transform<F>(&self, f: F) -> Self::Input
    where
        F: FnOnce(&Self::Input, Option<&Self::TransformParams>) -> Self::Input,
    {
        let Some(input) = &self.input() else {
            return Self::Input::default();
        };
        f(input, self.params().as_ref())
    }

    /// Expected output or error for the fixture.
    fn output(&self) -> Option<Result<Self::Output, Self::Error>>;
}

/// Builder that prepares a [`Fixture`] for execution.
#[derive(Clone, Debug)]
pub struct TestCase<D>
where
    D: Fixture + Clone,
{
    test_def: TestDef<D::Error>,
}

/// A fixture ready for the test runner: transformed input, HTTP mocks, and
/// expected output.
#[derive(Clone, Debug)]
pub struct PreparedTestCase<D>
where
    D: Fixture + Clone,
{
    /// Prepared input for the handler under test.
    pub input: Option<D::Input>,
    /// Optional HTTP request mocks required by the handler.
    pub http_requests: Option<Vec<Fetch>>,
    /// Expected output or error produced by the fixture.
    pub output: Option<Result<D::Output, D::Error>>,
}

impl<D> TestCase<D>
where
    D: Clone + Fixture,
{
    /// Create a new test case from the given fixture data.
    #[must_use]
    pub const fn new(test_def: TestDef<D::Error>) -> Self {
        Self { test_def }
    }

    /// Transform the input and extract mocks / expected output.
    pub fn prepare<F>(&self, transform: F) -> PreparedTestCase<D>
    where
        F: FnOnce(&D::Input, Option<&D::TransformParams>) -> D::Input,
    {
        let http_requests = self.test_def.http_requests.clone();
        let data = D::from_data(&self.test_def);
        let output = data.output();
        if data.input().is_none() {
            return PreparedTestCase { input: None, http_requests, output };
        }
        let input = data.transform(transform);
        PreparedTestCase { input: Some(input), http_requests, output }
    }
}

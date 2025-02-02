use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json};

use super::{Collection, Object, PartitionSelector, RelativeUrl};

/// A test step describes either an "ingest" of document fixtures into a
/// collection, or a "verify" of expected document fixtures from a collection.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(untagged)]
#[schemars(example = "TestDocuments::example_relative")]
#[schemars(example = "TestDocuments::example_inline")]
pub enum TestDocuments {
    /// Relative URL to a file of documents.
    Url(RelativeUrl),
    /// An inline array of documents.
    Inline(Vec<Object>),
}

impl TestDocuments {
    pub fn example_relative() -> Self {
        from_value(json!("../path/to/test-documents.json")).unwrap()
    }
    pub fn example_inline() -> Self {
        from_value(json!([
            {"a": "document"},
            {"another": "document"},
        ]))
        .unwrap()
    }
}

/// A test step describes either an "ingest" of document fixtures into a
/// collection, or a "verify" of expected document fixtures from a collection.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[schemars(example = "TestStep::example_ingest")]
#[schemars(example = "TestStep::example_verify")]
pub enum TestStep {
    /// Ingest document fixtures into a collection.
    Ingest(TestStepIngest),
    /// Verify the contents of a collection match a set of document fixtures.
    Verify(TestStepVerify),
}

impl TestStep {
    pub fn example_ingest() -> Self {
        TestStep::Ingest(TestStepIngest::example())
    }
    pub fn example_verify() -> Self {
        TestStep::Verify(TestStepVerify::example())
    }
}

/// An ingestion test step ingests document fixtures into the named
/// collection.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[schemars(example = "TestStepIngest::example")]
pub struct TestStepIngest {
    /// # Description of this test ingestion.
    #[serde(default)]
    pub description: String,
    /// # Name of the collection into which the test will ingest.
    pub collection: Collection,
    /// # Documents to ingest.
    /// Each document must conform to the collection's schema.
    pub documents: TestDocuments,
}

impl TestStepIngest {
    pub fn example() -> Self {
        Self {
            description: "Description of the ingestion.".to_string(),
            collection: Collection::example(),
            documents: TestDocuments::example_inline(),
        }
    }
}

/// A verification test step verifies that the contents of the named
/// collection match the expected fixtures, after fully processing all
/// preceding ingestion test steps.
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[schemars(example = "TestStepVerify::example")]
pub struct TestStepVerify {
    /// # Description of this test verification.
    #[serde(default)]
    pub description: String,
    /// # Collection into which the test will ingest.
    pub collection: Collection,
    /// # Documents to verify.
    /// Each document may contain only a portion of the matched document's
    /// properties, and any properties present in the actual document but
    /// not in this document fixture are ignored. All other values must
    /// match or the test will fail.
    pub documents: TestDocuments,
    /// # Selector over partitions to verify.
    #[serde(default)]
    #[schemars(default = "PartitionSelector::example")]
    pub partitions: Option<PartitionSelector>,
}

impl TestStepVerify {
    pub fn example() -> Self {
        Self {
            description: "Description of the verification.".to_string(),
            collection: Collection::example(),
            documents: TestDocuments::example_inline(),
            partitions: None,
        }
    }
}

use forge_core_contracts::MarkdownRetirementDocument;
use forge_core_validate::{validate_markdown_retirement, MARKDOWN_RETIREMENT_AUTHORITY_PATH};

#[test]
fn checked_in_markdown_authority_and_local_links_are_valid() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let authority = std::fs::read_to_string(root.join(MARKDOWN_RETIREMENT_AUTHORITY_PATH))
        .expect("read checked-in Markdown authority");
    let document = yaml_serde::from_str::<MarkdownRetirementDocument>(&authority)
        .expect("parse checked-in Markdown authority");

    let report = validate_markdown_retirement(&root, &document);
    assert!(
        report.diagnostics().is_empty(),
        "{:#?}",
        report.diagnostics()
    );
}

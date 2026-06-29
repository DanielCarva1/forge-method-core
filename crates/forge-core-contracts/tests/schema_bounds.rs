use forge_core_contracts::{
    AutonomyPolicyContractDocument, EvalRunContractDocument, MemoryContractDocument,
    TelemetryContractDocument,
};
use schemars::{schema_for, JsonSchema};
use serde_json::Value;

fn assert_field_has_bounds<T>(field_name: &str, minimum: u64, maximum: u64)
where
    T: JsonSchema,
{
    let schema = serde_json::to_value(schema_for!(T)).expect("schema serializes to JSON");
    let mut field_schemas = Vec::new();
    collect_field_schemas(&schema, field_name, &mut field_schemas);

    assert!(
        !field_schemas.is_empty(),
        "expected to find field '{field_name}' in schema:\n{schema:#}"
    );
    assert!(
        field_schemas
            .iter()
            .any(|field_schema| contains_numeric_bound(field_schema, "minimum", minimum)),
        "expected field '{field_name}' to expose minimum {minimum}; candidates:\n{field_schemas:#?}"
    );
    assert!(
        field_schemas
            .iter()
            .any(|field_schema| contains_numeric_bound(field_schema, "maximum", maximum)),
        "expected field '{field_name}' to expose maximum {maximum}; candidates:\n{field_schemas:#?}"
    );
}

fn collect_field_schemas<'a>(value: &'a Value, field_name: &str, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            if let Some(properties) = map.get("properties").and_then(Value::as_object) {
                if let Some(field_schema) = properties.get(field_name) {
                    found.push(field_schema);
                }
            }
            for nested in map.values() {
                collect_field_schemas(nested, field_name, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_field_schemas(item, field_name, found);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn contains_numeric_bound(value: &Value, key: &str, expected: u64) -> bool {
    match value {
        Value::Object(map) => {
            map.get(key)
                .is_some_and(|actual| numeric_equals(actual, expected))
                || map
                    .values()
                    .any(|nested| contains_numeric_bound(nested, key, expected))
        }
        Value::Array(items) => items
            .iter()
            .any(|item| contains_numeric_bound(item, key, expected)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn numeric_equals(value: &Value, expected: u64) -> bool {
    value.as_u64().is_some_and(|actual| actual == expected)
        || value
            .as_i64()
            .and_then(|actual| u64::try_from(actual).ok())
            .is_some_and(|actual| actual == expected)
}

#[test]
fn generated_schemas_expose_domain_bounds_for_percentage_like_fields() {
    assert_field_has_bounds::<AutonomyPolicyContractDocument>("risk_score", 0, 100);
    assert_field_has_bounds::<AutonomyPolicyContractDocument>("requires_approval_above", 0, 100);
    assert_field_has_bounds::<EvalRunContractDocument>("confidence", 0, 100);
    assert_field_has_bounds::<MemoryContractDocument>("confidence", 0, 100);
}

#[test]
fn generated_schemas_expose_per_myriad_sampling_bound() {
    assert_field_has_bounds::<TelemetryContractDocument>("rate", 0, 10_000);
}

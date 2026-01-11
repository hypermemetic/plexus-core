//! Serde helper utilities for consistent deserialization behavior
//!
//! These helpers are used by generated code to handle edge cases in JSON deserialization.

use serde::{Deserialize, Deserializer};

/// Deserializes an optional field that treats explicit `null` as `None`.
///
/// By default, serde with `#[serde(default)]` handles missing fields as `None`,
/// but explicit `null` values cause deserialization errors for `Option<T>` when
/// `T` doesn't accept null.
///
/// This deserializer treats both missing fields AND explicit `null` as `None`.
///
/// # Usage in generated code
///
/// ```ignore
/// #[serde(default, deserialize_with = "hub_core::serde_helpers::deserialize_null_as_none")]
/// field: Option<SomeType>
/// ```
///
/// # Behavior
///
/// - Missing field -> `None` (via `#[serde(default)]`)
/// - Explicit `null` -> `None`
/// - Present value -> `Some(value)`
pub fn deserialize_null_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    // Use Option<Option<T>> pattern: outer Option captures null vs present,
    // inner Option is what we actually return
    let opt = Option::<T>::deserialize(deserializer)?;
    Ok(opt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestStruct {
        #[serde(default, deserialize_with = "deserialize_null_as_none")]
        optional_string: Option<String>,

        #[serde(default, deserialize_with = "deserialize_null_as_none")]
        optional_number: Option<i32>,

        required: String,
    }

    #[test]
    fn test_missing_field() {
        let json = r#"{"required": "test"}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.optional_string, None);
        assert_eq!(parsed.optional_number, None);
        assert_eq!(parsed.required, "test");
    }

    #[test]
    fn test_explicit_null() {
        let json = r#"{"optional_string": null, "optional_number": null, "required": "test"}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.optional_string, None);
        assert_eq!(parsed.optional_number, None);
        assert_eq!(parsed.required, "test");
    }

    #[test]
    fn test_present_value() {
        let json = r#"{"optional_string": "hello", "optional_number": 42, "required": "test"}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.optional_string, Some("hello".to_string()));
        assert_eq!(parsed.optional_number, Some(42));
        assert_eq!(parsed.required, "test");
    }

    #[test]
    fn test_mixed_null_and_present() {
        let json = r#"{"optional_string": null, "optional_number": 42, "required": "test"}"#;
        let parsed: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.optional_string, None);
        assert_eq!(parsed.optional_number, Some(42));
        assert_eq!(parsed.required, "test");
    }

    // Test with a complex inner type
    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerType {
        value: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestWithComplexInner {
        #[serde(default, deserialize_with = "deserialize_null_as_none")]
        complex: Option<InnerType>,
    }

    #[test]
    fn test_complex_inner_missing() {
        let json = r#"{}"#;
        let parsed: TestWithComplexInner = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.complex, None);
    }

    #[test]
    fn test_complex_inner_null() {
        let json = r#"{"complex": null}"#;
        let parsed: TestWithComplexInner = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.complex, None);
    }

    #[test]
    fn test_complex_inner_present() {
        let json = r#"{"complex": {"value": "hello"}}"#;
        let parsed: TestWithComplexInner = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.complex,
            Some(InnerType {
                value: "hello".to_string()
            })
        );
    }
}

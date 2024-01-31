# Perstruct

`perstruct` is a crate that provides a macro to transform a Rust struct into a settings struct.
It facilitates the loading and saving of struct fields as key/value pairs. This is particularly
useful for application settings or configurations, where each setting can be individually
updated and persisted.

This approach offers significant advantages over whole-struct serialization and
deserialization, especially in scenarios where the application's configuration or user
preferences need to be flexible and resilient to changes.

The field values are serialized as json. Other formats may be supported in the future.

Below is an example demonstrating how to use the `perstruct` macro to manage user ferences in an application:

```rust
use perstruct::perstruct;

// Define a struct for user preferences.
// Apply the `perstruct` macro to enable key/value storage capabilities.
#[perstruct]
struct UserPreferences {
    // Define user preferences with custom keys for storage.
    pub ui_theme: UiTheme, // key is the same as the field name by default

    #[perstruct(key = "notifications_enabled")] // key can be overridden
    #[perstruct(default = true)] // default value can be specified
    pub enable_notifications: bool,

    // Use `perstruct(default_fn = "default_language")` to specify a function that returns  default value.
    #[perstruct(default_fn = "default_language")]
    pub language: String,

    // Use `perstruct(skip)` to exclude fields from being persisted.
    #[perstruct(skip)]
    pub cache: std::collections::HashMap<String, String>
}

#[derive(serde_derive::Serialize, serde_derive::Deserialize, Default, Debug, PartialEq)]
pub enum UiTheme {
    #[default]
    Dark,
    Light
}

fn default_language() -> String { "en".to_string() }

// Simulate loading preferences from a key-value store (like a database or config file).
// These might be values previously saved by the user.
let kv_store_simulation = vec![
    ("ui_theme", "\"Dark\""),
    ("notifications_enabled", "true"),
].into_iter().collect();
let result = UserPreferences::from_map(&kv_store_simulation);

// Access the loaded preferences, handle deserialization errors or unknown fields.
let mut preferences = result.value;
assert_eq!(preferences.ui_theme(), &UiTheme::Dark);
assert_eq!(preferences.enable_notifications(), true);
assert_eq!(preferences.language(), "en");
assert_eq!(result.deserialization_errors, vec![]);
assert!(result.unknown_fields.is_empty());

// Modify preferences using the auto-generated setters.
preferences.set_ui_theme(UiTheme::Light);
preferences.set_enable_notifications(false);

// Retrieve changes (dirty fields) to persist them.
let mut changes = preferences.perstruct_get_changes().unwrap();
changes.sort_by_key(|(k, _)| *k);
assert_eq!(
    changes,
    vec![
        ("language", "\"en\"".to_string()), // language was not loaded but defaulted to "en", so it should be included in the changes
        ("notifications_enabled", "false".to_string()),
        ("ui_theme", "\"Light\"".to_string()),
    ]
);

// Simulate saving the changes to the kv store.
// ...

// Mark changes as saved using `perstruct_saved`.
preferences.perstruct_saved();

// Verify that there are no unsaved changes.
assert_eq!(preferences.perstruct_get_changes().unwrap(), vec![]);
```

## Restrictions

The `perstruct` macro can only be applied to structs that meet the following requirements:

- All non-skipped field types must implement `serde::Serialize` and `serde::Deserialize`.
- All non-skipped fields must implement `Default` or have a default value specified using perstruct(default = ...)]`or`#[perstruct(default_fn = "...")]`.

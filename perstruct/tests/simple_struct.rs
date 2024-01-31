use std::collections::HashSet;

use perstruct::{perstruct, PerstructLoadResult};

#[perstruct]
struct MySettings {
    #[perstruct(key = "b")]
    pub a: i32,
    #[perstruct(default_fn = "default_foo")]
    foo: Foo,
    #[perstruct(default = 2)]
    bar: i32,

    list: Vec<()>,
}

#[derive(PartialEq, Eq, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
struct Foo {}
fn default_foo() -> Foo {
    Foo {}
}

#[test]
fn some_basic_tests() {
    use pretty_assertions::assert_eq;

    let mut settings = MySettings::default();
    assert_eq!(settings.a(), 0);
    assert_eq!(settings.bar(), 2);
    assert_eq!(settings.foo(), &Foo {});

    settings.set_a(1);
    settings.set_bar(7);
    settings.set_bar(8);
    assert_eq!(settings.a(), 1);
    assert_eq!(
        settings.perstruct_dirty_fields(),
        &vec!["b", "bar"].into_iter().collect::<HashSet<_>>()
    );

    settings.update_list(|list| list.push(()));

    assert_eq!(
        MySettings::perstruct_keys(),
        vec!["b", "foo", "bar", "list"]
    );

    let PerstructLoadResult {
        value: settings,
        mut deserialization_errors,
        unknown_fields,
    } = MySettings::from_map(
        &vec![
            ("b", "3".to_string()),
            ("foo", "null".to_string()),
            ("bar", r#""a""#.to_string()),
            ("whatever", "null".to_string()),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(settings.a(), 3);
    assert_eq!(settings.bar(), 2);
    assert_eq!(settings.foo(), &Foo {});
    deserialization_errors.sort_by_key(|(k, _)| *k);
    assert_eq!(
        deserialization_errors,
        vec![
            (
                "bar",
                "invalid type: string \"a\", expected i32 at line 1 column 3".to_string()
            ),
            (
                "foo",
                "invalid type: null, expected struct Foo at line 1 column 4".to_string()
            ),
        ]
    );

    assert_eq!(unknown_fields, vec!["whatever".to_string()]);
}

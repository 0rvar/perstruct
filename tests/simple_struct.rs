use perstruct::settings;

#[settings]
struct MySettings {
    #[setting(key = "b")]
    pub a: i32,
    #[setting(default_fn = "default_foo")]
    foo: Foo,
    #[setting(default = 2)]
    bar: i32,
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
    assert_eq!(settings.a(), 1);
    assert_eq!(settings.perstruct_dirty_fields(), &["b", "bar"]);

    assert_eq!(MySettings::perstruct_keys(), vec!["b", "foo", "bar"]);

    let (settings, mut errors) = MySettings::from_map(
        &vec![
            ("b".to_string(), "3".to_string()),
            ("foo".to_string(), "null".to_string()),
            ("bar".to_string(), r#""a""#.to_string()),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(settings.a(), 3);
    assert_eq!(settings.bar(), 2);
    assert_eq!(settings.foo(), &Foo {});
    errors.sort_by_key(|(k, _)| *k);
    assert_eq!(
        errors,
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
}

use serde_json::Value;

fn read(path: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

#[test]
fn marketplace_points_at_the_plugin_directory() {
    let marketplace = read_json(".claude-plugin/marketplace.json");
    assert_eq!(marketplace["name"], "skald");
    let plugins = marketplace["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], "code-skillset");
    assert_eq!(plugins[0]["source"], "./plugin");

    let plugin = read_json("plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["metadata"]["skaldMinVersion"], "0.1.0");
}

#[test]
fn plugin_content_lives_under_the_plugin_directory() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for dir in ["skills", "hooks"] {
        assert!(
            !root.join(dir).exists(),
            "`{dir}/` at the repository root is outside the plugin; move it to `plugin/{dir}/`"
        );
    }
}

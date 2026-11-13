use std::fs;
use std::path::Path;

#[test]
fn test_packaging_metadata_and_templates_validity() {
    let pkg_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("pkg");
    assert!(pkg_dir.exists(), "pkg directory must exist");

    // 1. PKGBUILD checks
    let pkgbuild_path = pkg_dir.join("PKGBUILD");
    assert!(pkgbuild_path.exists());
    let pkgbuild = fs::read_to_string(&pkgbuild_path).unwrap();
    assert!(pkgbuild.contains("pkgname=soundthis"));
    assert!(pkgbuild.contains("arch=('x86_64' 'aarch64')"));
    assert!(pkgbuild.contains("install=soundthis.install"));
    assert!(pkgbuild.contains("depends=('gcc-libs' 'glibc' 'libpulse')"));
    assert!(pkgbuild.contains("/usr/bin/soundthis"));
    assert!(pkgbuild.contains("/usr/lib/systemd/user/soundthis.service"));
    assert!(pkgbuild.contains("/usr/lib/systemd/user-preset/90-soundthis.preset"));
    assert!(pkgbuild.contains("/usr/share/applications/soundthis.desktop"));
    assert!(pkgbuild.contains("/usr/share/icons/hicolor/scalable/apps/soundthis.svg"));
    assert!(pkgbuild.contains("/usr/share/omarchy/shell/plugins/soundthis/"));

    // 2. Desktop file checks
    let desktop_path = pkg_dir.join("soundthis.desktop");
    assert!(desktop_path.exists());
    let desktop = fs::read_to_string(&desktop_path).unwrap();
    assert!(desktop.contains("[Desktop Entry]"));
    assert!(desktop.contains("Type=Application"));
    assert!(desktop.contains("Name=SoundThis"));
    assert!(desktop.contains("Exec=/usr/bin/soundthis open"));
    assert!(desktop.contains("Icon=soundthis"));

    // 3. Systemd preset checks
    let preset_path = pkg_dir.join("90-soundthis.preset");
    assert!(preset_path.exists());
    let preset = fs::read_to_string(&preset_path).unwrap();
    assert!(preset.contains("enable soundthis.service"));

    // 4. SVG icon checks
    let svg_path = pkg_dir.join("soundthis.svg");
    assert!(svg_path.exists());
    let svg = fs::read_to_string(&svg_path).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));

    // 5. soundthis.install checks
    let install_path = pkg_dir.join("soundthis.install");
    assert!(install_path.exists());
    let install_sh = fs::read_to_string(&install_path).unwrap();
    assert!(install_sh.contains("post_install()"));
    assert!(install_sh.contains("post_upgrade()"));
    assert!(install_sh.contains("pre_remove()"));
    assert!(install_sh.contains("post_remove()"));
    assert!(install_sh.contains("soundthis.service"));
    assert!(install_sh.contains("enablePlugin soundthis"));
    assert!(install_sh.contains("setPluginEnabled soundthis false"));
}

#[test]
fn test_soundthis_install_scriptlet_lifecycle_simulation() {
    let pkg_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("pkg");
    let install_script = pkg_dir.join("soundthis.install");
    assert!(install_script.exists());

    let temp_dir = std::env::temp_dir().join(format!("soundthis_pkg_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fake_home = temp_dir.join("mock_home");
    let omarchy_config = fake_home.join(".config/omarchy");
    fs::create_dir_all(&omarchy_config).unwrap();

    let shell_json_path = omarchy_config.join("shell.json");
    let initial_shell = serde_json::json!({
        "bar": {
            "layout": {
                "right": [
                    { "id": "omarchy.tray" }
                ]
            }
        }
    });
    fs::write(
        &shell_json_path,
        serde_json::to_string(&initial_shell).unwrap(),
    )
    .unwrap();

    let test_runner = format!(
        r#"
        set -eu
        source "{install_script}"

        _get_target_users() {{
            echo "1000"
        }}
        id() {{
            if [[ "${{1:-}}" == "-nu" ]]; then
                echo "mockuser"
            else
                /usr/bin/id "$@"
            fi
        }}
        getent() {{
            echo "mockuser:x:1000:1000:Mock:{fake_home}:/bin/bash"
        }}

        # 1. Execute post_install
        post_install

        # Verify insertion into shell.json
        jq -e 'any(.bar.layout.right[]; .id == "soundthis" or . == "soundthis")' "{shell_json}" >/dev/null

        # 2. Execute post_install a second time to ensure idempotency
        post_install
        count=$(jq '[.bar.layout.right[] | select(.id == "soundthis" or . == "soundthis")] | length' "{shell_json}")
        if [ "$count" -ne 1 ]; then
            echo "Expected exactly 1 soundthis entry, found $count"
            exit 1
        fi

        # 3. Execute pre_remove
        pre_remove

        # Verify removal from shell.json
        if jq -e 'any(.bar.layout.right[]; .id == "soundthis" or . == "soundthis")' "{shell_json}" >/dev/null 2>&1; then
            echo "soundthis was not removed from shell.json"
            exit 1
        fi

        # 4. Execute post_remove
        post_remove
        "#,
        install_script = install_script.display(),
        fake_home = fake_home.display(),
        shell_json = shell_json_path.display(),
    );

    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(&test_runner)
        .output()
        .expect("Failed to execute bash runner");

    assert!(
        output.status.success(),
        "Scriptlet test failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_package_staging_and_filesystem_layout() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pkg_dir = manifest_dir.join("pkg");

    let required_files = [
        pkg_dir.join("PKGBUILD"),
        pkg_dir.join("soundthis.install"),
        pkg_dir.join("soundthis.desktop"),
        pkg_dir.join("90-soundthis.preset"),
        pkg_dir.join("soundthis.svg"),
        manifest_dir.join("plugin/manifest.json"),
        manifest_dir.join("plugin/Panel.qml"),
        manifest_dir.join("systemd/soundthis.service.template"),
        manifest_dir.join("config/config.toml.template"),
    ];

    for file in &required_files {
        assert!(file.exists(), "Required package file missing: {:?}", file);
    }
}

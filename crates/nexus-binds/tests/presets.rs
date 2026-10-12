//! `find_presets` / `load_effective` against a synthetic bindings folder on disk.

use std::fs;
use std::path::{Path, PathBuf};

use nexus_binds::{PresetCategory, Version, analyze, find_presets, render_html};

fn fresh_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn binds(name: &str, key: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" ?>
<Root PresetName="{name}" MajorVersion="4" MinorVersion="2">
	<KeyboardLayout>en-US</KeyboardLayout>
	<LandingGearToggle>
		<Primary Device="Keyboard" Key="{key}" />
		<Secondary Device="{{NoDevice}}" Key="" />
	</LandingGearToggle>
	<HumanoidJumpButton>
		<Primary Device="Keyboard" Key="{key}" />
		<Secondary Device="{{NoDevice}}" Key="" />
	</HumanoidJumpButton>
</Root>
"#
    )
}

#[test]
fn discovers_newest_versions_and_active_presets() {
    let dir = fresh_dir("presets-discovery");
    fs::write(dir.join("Custom.3.0.binds"), binds("Custom", "Key_A")).unwrap();
    fs::write(dir.join("Custom.4.0.binds"), binds("Custom", "Key_B")).unwrap();
    fs::write(dir.join("Custom.4.2.binds"), binds("Custom", "Key_C")).unwrap();
    // A newer major than the start file's: the 4.x client does not read it.
    fs::write(dir.join("Custom.5.0.binds"), binds("Custom", "Key_D")).unwrap();
    fs::write(dir.join("Mine.4.1.binds"), binds("Mine", "Key_M")).unwrap();
    fs::write(dir.join("StartPreset.start"), "Custom\n").unwrap();
    fs::write(
        dir.join("StartPreset.4.start"),
        "Mine\r\nMine\r\nCustom\r\nKeyboardMouseOnly\r\n",
    )
    .unwrap();
    fs::write(dir.join("notes.txt"), "not a preset").unwrap();
    fs::create_dir_all(dir.join("Folder.4.2.binds")).unwrap();

    let found = find_presets(&dir).unwrap();
    let names: Vec<&str> = found.presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["Mine", "Custom"],
        "active presets first, by category"
    );
    assert_eq!(
        found.start_preset_path.as_deref(),
        Some(dir.join("StartPreset.4.start").as_path())
    );

    let custom = found.get("Custom").unwrap();
    assert_eq!(custom.version, Some(Version { major: 4, minor: 2 }));
    assert_eq!(custom.path, dir.join("Custom.4.2.binds"));
    assert_eq!(custom.older_versions.len(), 3);
    assert_eq!(custom.active_for, [PresetCategory::Srv]);

    let mine = found.get("Mine").unwrap();
    assert_eq!(
        mine.active_for,
        [PresetCategory::General, PresetCategory::Ship]
    );
    assert!(mine.is_active());
    assert_eq!(found.primary().unwrap().name, "Mine");
    assert_eq!(
        found.active_name(PresetCategory::OnFoot),
        Some("KeyboardMouseOnly")
    );
    assert!(
        found.active(PresetCategory::OnFoot).is_none(),
        "built-in presets live elsewhere"
    );

    // Without the game's ControlSchemes folder the built-in on-foot preset cannot be resolved…
    assert!(found.load_effective(&[]).is_err());

    // …with it, every category comes from its own preset.
    let builtin = fresh_dir("presets-builtin");
    fs::write(
        builtin.join("KeyboardMouseOnly.binds"),
        binds("KeyboardMouseOnly", "Key_Space"),
    )
    .unwrap();
    assert_eq!(
        found.resolve("KeyboardMouseOnly", &[&builtin]),
        Some(builtin.join("KeyboardMouseOnly.binds"))
    );
    let effective = found.load_effective(&[&builtin]).unwrap();
    assert_eq!(effective.preset_name, "Mine + Custom + KeyboardMouseOnly");
    let key = |action: &str| {
        effective
            .action(action)
            .unwrap()
            .primary
            .as_ref()
            .unwrap()
            .key
            .clone()
    };
    assert_eq!(key("LandingGearToggle"), "Key_M");
    assert_eq!(key("HumanoidJumpButton"), "Key_Space");

    // The whole pipeline works on the result.
    let report = analyze(&effective);
    assert!(report.conflicts.is_empty());
    assert!(render_html(&effective, &report).contains("Active presets:"));
}

#[test]
fn missing_folder_is_an_error() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("presets-does-not-exist");
    assert!(find_presets(dir).is_err());
}

#[test]
fn folder_without_start_file_uses_newest_preset() {
    let dir = fresh_dir("presets-no-start");
    fs::write(dir.join("Solo.4.2.binds"), binds("Solo", "Key_S")).unwrap();
    let found = find_presets(&dir).unwrap();
    assert!(found.start_preset.is_none());
    assert!(!found.presets[0].is_active());
    assert_eq!(found.primary().unwrap().name, "Solo");
    assert_eq!(found.load_effective(&[]).unwrap().preset_name, "Solo");
}

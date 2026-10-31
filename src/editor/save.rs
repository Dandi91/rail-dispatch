//! Persist the edited model back to `resources/level.toml`.
//!
//! Uses `toml_edit` to surgically replace only the `geometry`, `blocks`, and `connections`
//! arrays, leaving every other table (switches, signals, sections, stations, spawners,
//! background) and the file's comments untouched. `Ctrl+S` triggers a save.

use bevy::asset::io::file::FileAssetReader;
use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::state::EditorModel;

/// The level file, relative to the asset base path (matches `AssetPlugin.file_path` in `main.rs`).
const LEVEL_PATH: &str = "resources/level.toml";

/// Resolves [`LEVEL_PATH`] the same way Bevy resolves the asset root (`BEVY_ASSET_ROOT`, then
/// `CARGO_MANIFEST_DIR`, then the executable's directory), so the editor saves the file it loaded
/// regardless of the current working directory.
fn level_path() -> PathBuf {
    FileAssetReader::get_base_path().join(LEVEL_PATH)
}

pub fn save_system(keys: Res<ButtonInput<KeyCode>>, model: Res<EditorModel>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        let path = level_path();
        match save(&model, &path) {
            Ok(()) => info!("saved {}", path.display()),
            Err(e) => error!("saving {} failed: {e}", path.display()),
        }
    }
}

fn save(model: &EditorModel, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let mut doc = text.parse::<DocumentMut>()?;

    doc["blocks"] = Item::Value(blocks_array(model).into());
    doc["connections"] = Item::Value(connections_array(model).into());
    doc["geometry"] = Item::Value(geometry_array(model).into());

    // Write a sibling temp file and rename it over the level, so a crash mid-write can't leave a
    // truncated level behind (rename within one directory is atomic).
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, doc.to_string())?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Render an integer if the float is whole, else keep the float (matches the all-integer source).
fn number(v: f64) -> Value {
    if v.fract() == 0.0 {
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "v is whole; block lengths are far inside i64 range"
        )]
        Value::from(v as i64)
    } else {
        Value::from(v)
    }
}

#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "rounded pixel coordinates are far inside i64 range"
)]
fn coord(v: f32) -> Value {
    Value::from(v.round() as i64)
}

fn blocks_array(model: &EditorModel) -> Array {
    let mut arr = Array::new();
    for b in &model.blocks {
        let mut row = Array::new();
        row.push(i64::from(b.id));
        row.push(number(b.length));
        arr.push(row);
    }
    multiline(arr)
}

fn connections_array(model: &EditorModel) -> Array {
    let mut arr = Array::new();
    for &(start, end) in &model.connections {
        let mut row = Array::new();
        row.push(i64::from(start));
        row.push(i64::from(end));
        arr.push(row);
    }
    multiline(arr)
}

fn geometry_array(model: &EditorModel) -> Array {
    let mut arr = Array::new();
    for g in &model.geometry {
        let mut points = Array::new();
        for p in &g.points {
            let mut pt = Array::new();
            pt.push(coord(p.x));
            pt.push(coord(p.y));
            points.push(pt);
        }
        let mut row = Array::new();
        row.push(i64::from(g.id));
        row.push(points);
        arr.push(row);
    }
    multiline(arr)
}

/// Put each top-level row on its own indented line, e.g. `[\n    [..],\n    [..],\n]`.
fn multiline(mut arr: Array) -> Array {
    for v in arr.iter_mut() {
        v.decor_mut().set_prefix("\n    ");
    }
    arr.set_trailing("\n");
    arr.set_trailing_comma(true);
    arr
}

#[cfg(test)]
mod tests {
    use super::*;
    use rail_dispatch::level::Level;

    const REFERENCE_LEVEL: &str = include_str!("../../tests/fixtures/reference_level.toml");

    #[test]
    fn save_round_trips_and_keeps_other_tables() {
        let level: Level = toml::from_str(REFERENCE_LEVEL).expect("reference level parses");
        let mut model = EditorModel::from_level(&level);
        model.geometry[0].points[0].x -= 12.0;

        let dir = std::env::temp_dir().join(format!("rail-dispatch-save-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("level.toml");
        fs::write(&path, REFERENCE_LEVEL).unwrap();

        save(&model, &path).expect("save succeeds");
        let saved = fs::read_to_string(&path).unwrap();
        let reloaded: Level = toml::from_str(&saved).expect("saved level parses");
        let leftover_tmp = path.with_extension("toml.tmp").exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(reloaded.validate(), Ok(()));
        assert_eq!(reloaded.geometry[0].points[0], model.geometry[0].points[0]);
        assert_eq!(reloaded.stations.len(), level.stations.len());
        assert!(saved.contains("# id, signal_id, section id list"), "comments preserved");
        assert!(!leftover_tmp, "temp file renamed away");
    }
}

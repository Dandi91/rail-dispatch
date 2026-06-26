//! Persist the edited model back to `resources/level.toml`.
//!
//! Uses `toml_edit` to surgically replace only the `geometry`, `blocks`, and `connections`
//! arrays, leaving every other table (switches, signals, sections, stations, spawners,
//! background) and the file's comments untouched. `Ctrl+S` triggers a save.

use bevy::prelude::*;
use std::fs;
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::state::EditorModel;

const LEVEL_PATH: &str = "resources/level.toml";

pub fn save_system(keys: Res<ButtonInput<KeyCode>>, model: Res<EditorModel>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        match save(&model) {
            Ok(()) => info!("saved {LEVEL_PATH}"),
            Err(e) => error!("save failed: {e}"),
        }
    }
}

fn save(model: &EditorModel) -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(LEVEL_PATH)?;
    let mut doc = text.parse::<DocumentMut>()?;

    doc["blocks"] = Item::Value(blocks_array(model).into());
    doc["connections"] = Item::Value(connections_array(model).into());
    doc["geometry"] = Item::Value(geometry_array(model).into());

    fs::write(LEVEL_PATH, doc.to_string())?;
    Ok(())
}

/// Render an integer if the float is whole, else keep the float (matches the all-integer source).
fn number(v: f64) -> Value {
    if v.fract() == 0.0 {
        Value::from(v as i64)
    } else {
        Value::from(v)
    }
}

fn coord(v: f32) -> Value {
    Value::from(v.round() as i64)
}

fn blocks_array(model: &EditorModel) -> Array {
    let mut arr = Array::new();
    for b in &model.blocks {
        let mut row = Array::new();
        row.push(b.id as i64);
        row.push(number(b.length));
        arr.push(row);
    }
    multiline(arr)
}

fn connections_array(model: &EditorModel) -> Array {
    let mut arr = Array::new();
    for &(start, end) in &model.connections {
        let mut row = Array::new();
        row.push(start as i64);
        row.push(end as i64);
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
        row.push(g.id as i64);
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

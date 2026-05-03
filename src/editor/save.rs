use rail_dispatch::level::LampData;
use std::error::Error;
use std::fs;

const LEVEL_PATH: &str = "resources/level.toml";

pub fn save_level(lamps: &[LampData]) -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(LEVEL_PATH)?;
    let mut value: toml::Value = toml::from_str(&contents)?;

    let mut arr = toml::value::Array::new();
    for lamp in lamps {
        let mut row = toml::value::Array::new();
        row.push(num(lamp.id as f64));
        row.push(num(lamp.x as f64));
        row.push(num(lamp.y as f64));
        row.push(num(lamp.width as f64));
        if lamp.rotation != 0.0 {
            row.push(num(lamp.rotation as f64));
        }
        arr.push(toml::Value::Array(row));
    }

    if let toml::Value::Table(t) = &mut value {
        t.insert("lamps".into(), toml::Value::Array(arr));
    } else {
        return Err("level.toml root is not a table".into());
    }

    let new_contents = toml::to_string_pretty(&value)?;
    fs::write(LEVEL_PATH, new_contents)?;
    Ok(())
}

fn num(v: f64) -> toml::Value {
    if v.fract() == 0.0 && v.abs() < (i64::MAX as f64) {
        toml::Value::Integer(v as i64)
    } else {
        toml::Value::Float(v)
    }
}

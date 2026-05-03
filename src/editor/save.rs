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
        row.push(toml::Value::Integer(lamp.id as i64));
        row.push(toml::Value::Integer(lamp.x as i64));
        row.push(toml::Value::Integer(lamp.y as i64));
        row.push(toml::Value::Integer(lamp.width as i64));
        if lamp.rotation != 0 {
            row.push(toml::Value::Integer(lamp.rotation as i64));
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

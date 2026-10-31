use crate::common::{
    BlockId, Direction, HexColor, RouteId, SectionId, SignalId, SignalType, StationId, SwitchId, SwitchPosition,
};
use bevy::{asset::AssetLoader, asset::LoadContext, asset::io::Reader, prelude::*};
use futures_lite::AsyncReadExt;
use serde::Deserialize;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Deserialize, Asset, Reflect, Debug)]
pub struct Level {
    pub blocks: Vec<BlockData>,
    pub connections: Vec<ConnectionData>,
    pub switches: Vec<SwitchData>,
    pub spawners: Vec<SpawnerData>,
    pub signals: Vec<SignalData>,
    #[serde(default)]
    pub sections: Vec<SectionData>,
    #[serde(default)]
    pub stations: Vec<StationData>,
    #[serde(default)]
    pub geometry: Vec<BlockGeometry>,
    /// Vertical spacing (pixel space) between schematic rows. The editor snaps track vertices to
    /// an infinite grid at multiples of this value, in both directions.
    #[serde(default = "default_row_spacing")]
    pub row_spacing: f32,
    pub background: HexColor,
}

fn default_row_spacing() -> f32 {
    12.0
}

/// Schematic geometry for a block: a polyline (in level pixel space) along which the
/// block's track is drawn. The panel renderer maps this to world space.
#[derive(Deserialize, Reflect, Debug)]
pub struct BlockGeometry {
    pub id: BlockId,
    pub points: Vec<Vec2>,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct BlockData {
    pub id: BlockId,
    pub length: f64,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct ConnectionData {
    pub start: BlockId,
    pub end: BlockId,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct SwitchData {
    pub id: SwitchId,
    pub base: BlockId,
    pub straight: BlockId,
    pub side: BlockId,
    pub direction: Direction,
}

#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Reflect)]
#[serde(rename_all = "lowercase")]
#[derive(Debug)]
pub enum SpawnerKind {
    Spawn,
    Despawn,
    Both,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct SpawnerData {
    pub block_id: BlockId,
    pub kind: SpawnerKind,
    #[serde(default)]
    pub approach_len: u8,
    #[serde(default)]
    pub speed_kmh: f64,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct SignalData {
    pub id: SignalId,
    pub block_id: BlockId,
    pub offset_m: f64,
    pub name: String,
    pub direction: Direction,
    #[serde(default)]
    pub signal_type: SignalType,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct SectionData {
    pub id: SectionId,
    pub blocks: Vec<BlockId>,
}

#[derive(Deserialize, Reflect, Clone, Debug)]
pub struct SwitchSetting {
    pub switch_id: SwitchId,
    pub position: SwitchPosition,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct RouteData {
    pub id: RouteId,
    pub signal: SignalId,
    pub sections: Vec<SectionId>,
    pub target: BlockId,
    #[serde(default)]
    pub switches: Vec<SwitchSetting>,
}

#[derive(Deserialize, Reflect, Debug)]
pub struct StationData {
    pub id: StationId,
    pub name: String,
    pub routes: Vec<RouteData>,
}

#[derive(Debug)]
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Level>().register_asset_loader(LevelLoader);
    }
}

#[derive(TypePath)]
struct LevelLoader;

/// Bevy logs only the `Display` of a loader error (it doesn't walk `source()`), so each variant
/// includes its inner error in the message.
#[derive(Debug, Error)]
pub enum LevelLoaderError {
    #[error("failed to read level file: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse level file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("level is invalid: {0}")]
    Invalid(#[from] LevelValidationError),
}

/// A cross-reference problem in an otherwise well-formed level file.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LevelValidationError {
    #[error("{table}: duplicate {kind} id {id}")]
    Duplicate {
        table: &'static str,
        kind: &'static str,
        id: u32,
    },
    #[error("{table}: unknown {kind} id {id}")]
    DanglingRef {
        table: &'static str,
        kind: &'static str,
        id: u32,
    },
}

impl Level {
    /// Checks that every id the level refers to is declared exactly once, so the simulation can
    /// index its containers without panicking on bad data.
    ///
    /// # Errors
    ///
    /// Returns the first duplicate declaration or dangling reference found.
    pub fn validate(&self) -> Result<(), LevelValidationError> {
        let blocks = unique_ids("blocks", "block", self.blocks.iter().map(|b| b.id))?;
        let switches = unique_ids("switches", "switch", self.switches.iter().map(|s| s.id))?;
        let signals = unique_ids("signals", "signal", self.signals.iter().map(|s| s.id))?;
        let sections = unique_ids("sections", "section", self.sections.iter().map(|s| s.id))?;
        let routes = self.stations.iter().flat_map(|s| &s.routes);
        unique_ids("stations", "route", routes.clone().map(|r| r.id))?;

        let check = |table, kind, known: &HashSet<u32>, id: u32| {
            if known.contains(&id) {
                Ok(())
            } else {
                Err(LevelValidationError::DanglingRef { table, kind, id })
            }
        };
        for c in &self.connections {
            check("connections", "block", &blocks, c.start)?;
            check("connections", "block", &blocks, c.end)?;
        }
        for s in &self.switches {
            for id in [s.base, s.straight, s.side] {
                check("switches", "block", &blocks, id)?;
            }
        }
        for s in &self.spawners {
            check("spawners", "block", &blocks, s.block_id)?;
        }
        for s in &self.signals {
            check("signals", "block", &blocks, s.block_id)?;
        }
        for s in &self.sections {
            for &id in &s.blocks {
                check("sections", "block", &blocks, id)?;
            }
        }
        for r in routes {
            check("stations", "signal", &signals, r.signal)?;
            check("stations", "block", &blocks, r.target)?;
            for &id in &r.sections {
                check("stations", "section", &sections, id)?;
            }
            for s in &r.switches {
                check("stations", "switch", &switches, s.switch_id)?;
            }
        }
        for g in &self.geometry {
            check("geometry", "block", &blocks, g.id)?;
        }
        Ok(())
    }
}

fn unique_ids(
    table: &'static str,
    kind: &'static str,
    ids: impl Iterator<Item = u32>,
) -> Result<HashSet<u32>, LevelValidationError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(LevelValidationError::Duplicate { table, kind, id });
        }
    }
    Ok(seen)
}

impl AssetLoader for LevelLoader {
    type Asset = Level;
    type Settings = ();
    type Error = LevelLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut contents = String::new();
        reader.read_to_string(&mut contents).await?;
        let level: Level = toml::from_str(&contents)?;
        level.validate()?;
        Ok(level)
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED_LEVEL: &str = include_str!("../resources/level.toml");

    #[test]
    fn shipped_level_is_valid() {
        let level: Level = toml::from_str(SHIPPED_LEVEL).expect("shipped level parses");
        assert_eq!(level.validate(), Ok(()));
    }

    #[test]
    fn dangling_connection_is_rejected() {
        let mut level: Level = toml::from_str(SHIPPED_LEVEL).expect("shipped level parses");
        level.connections.push(ConnectionData { start: 1, end: 9999 });
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::DanglingRef {
                table: "connections",
                kind: "block",
                id: 9999
            })
        );
    }

    #[test]
    fn duplicate_block_is_rejected() {
        let mut level: Level = toml::from_str(SHIPPED_LEVEL).expect("shipped level parses");
        let id = level.blocks[0].id;
        level.blocks.push(BlockData { id, length: 1.0 });
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::Duplicate {
                table: "blocks",
                kind: "block",
                id
            })
        );
    }
}

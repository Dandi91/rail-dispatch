use crate::common::{
    BlockId, Direction, HexColor, RouteId, SectionId, SignalId, SignalType, StationId, SwitchId, SwitchPosition,
};
use bevy::{asset::AssetLoader, asset::LoadContext, asset::io::Reader, prelude::*};
use futures_lite::AsyncReadExt;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
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
    /// Sections are track circuits: a block belongs to at most one of them.
    #[error("sections: block {block} is in both section {first} and section {second}")]
    SharedBlock {
        block: BlockId,
        first: SectionId,
        second: SectionId,
    },
    #[error("sections: section {0} is not one connected stretch of track")]
    DisconnectedSection(SectionId),
    #[error("stations: route {route}: {error}")]
    RoutePath { route: RouteId, error: RoutePathError },
}

/// Why a route's path can't be traced from its signal to its target.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RoutePathError {
    #[error("unknown signal {0}")]
    UnknownSignal(SignalId),
    #[error("passes switch {0} without setting its position")]
    UnsetSwitch(SwitchId),
    #[error("enters switch {0} from the leg it is not set to")]
    SwitchSetAgainst(SwitchId),
    #[error("track ends after block {0}")]
    DeadEnd(BlockId),
    #[error("leaves the route's sections at block {0}")]
    LeavesSections(BlockId),
    #[error("does not reach the target block")]
    TargetNotReached,
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

        self.validate_sections()?;
        for route in self.stations.iter().flat_map(|s| &s.routes) {
            self.route_path(route)
                .map_err(|error| LevelValidationError::RoutePath { route: route.id, error })?;
        }
        Ok(())
    }

    /// Sections must not share blocks, and each must be one connected stretch of track when every
    /// switch leg counts as connected (a switch section covers both legs).
    fn validate_sections(&self) -> Result<(), LevelValidationError> {
        let mut owner: HashMap<BlockId, SectionId> = HashMap::new();
        for section in &self.sections {
            for &block in &section.blocks {
                if let Some(first) = owner.insert(block, section.id) {
                    return Err(LevelValidationError::SharedBlock {
                        block,
                        first,
                        second: section.id,
                    });
                }
            }
        }

        let mut neighbours: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        let links = self.connections.iter().map(|c| (c.start, c.end)).chain(
            self.switches
                .iter()
                .flat_map(|s| [(s.base, s.straight), (s.base, s.side)]),
        );
        for (a, b) in links {
            neighbours.entry(a).or_default().push(b);
            neighbours.entry(b).or_default().push(a);
        }

        for section in &self.sections {
            let members: HashSet<BlockId> = section.blocks.iter().copied().collect();
            let Some(&first) = section.blocks.first() else {
                continue;
            };
            let mut reached = HashSet::from([first]);
            let mut stack = vec![first];
            while let Some(block) = stack.pop() {
                for &next in neighbours.get(&block).into_iter().flatten() {
                    if members.contains(&next) && reached.insert(next) {
                        stack.push(next);
                    }
                }
            }
            if reached.len() != members.len() {
                return Err(LevelValidationError::DisconnectedSection(section.id));
            }
        }
        Ok(())
    }

    /// The blocks a route's train runs over, from the block after its signal up to and including its
    /// target, following the route's switch positions. The route's signal is taken to stand at the
    /// end of its block, facing the route.
    ///
    /// # Errors
    ///
    /// Returns why the path can't be traced: a switch on the way the route doesn't set (or sets
    /// against the path), a dead end, or a block outside the route's sections before the target.
    pub fn route_path(&self, route: &RouteData) -> Result<Vec<BlockId>, RoutePathError> {
        let signal = self
            .signals
            .iter()
            .find(|s| s.id == route.signal)
            .ok_or(RoutePathError::UnknownSignal(route.signal))?;
        let positions: HashMap<SwitchId, SwitchPosition> =
            route.switches.iter().map(|s| (s.switch_id, s.position)).collect();
        let allowed: HashSet<BlockId> = self
            .sections
            .iter()
            .filter(|s| route.sections.contains(&s.id))
            .flat_map(|s| s.blocks.iter().copied())
            .collect();

        let mut path = Vec::new();
        let mut block = signal.block_id;
        // Every step lands on a new route block, so a longer walk means the path loops.
        for _ in 0..=allowed.len() {
            let next = self
                .next_block_with(block, signal.direction, &positions)?
                .ok_or(RoutePathError::DeadEnd(block))?;
            path.push(next);
            if next == route.target {
                return Ok(path);
            }
            if !allowed.contains(&next) {
                return Err(RoutePathError::LeavesSections(next));
            }
            block = next;
        }
        Err(RoutePathError::TargetNotReached)
    }

    /// The block after `block` in `direction`, with switches set as in `positions`.
    fn next_block_with(
        &self,
        block: BlockId,
        direction: Direction,
        positions: &HashMap<SwitchId, SwitchPosition>,
    ) -> Result<Option<BlockId>, RoutePathError> {
        let position = |id| positions.get(&id).copied().ok_or(RoutePathError::UnsetSwitch(id));
        for s in &self.switches {
            let active_leg = |position| match position {
                SwitchPosition::Straight => s.straight,
                SwitchPosition::Side => s.side,
            };
            if s.base == block && s.direction == direction {
                return Ok(Some(active_leg(position(s.id)?)));
            }
            if (s.straight == block || s.side == block) && s.direction == direction.reverse() {
                return if active_leg(position(s.id)?) == block {
                    Ok(Some(s.base))
                } else {
                    Err(RoutePathError::SwitchSetAgainst(s.id))
                };
            }
        }
        Ok(self.connections.iter().find_map(|c| match direction {
            Direction::Even => (c.start == block).then_some(c.end),
            Direction::Odd => (c.end == block).then_some(c.start),
        }))
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
    /// Frozen copy of the map that tests may refer to by id; the shipped level is free to change.
    const REFERENCE_LEVEL: &str = include_str!("../tests/fixtures/reference_level.toml");

    fn reference_level() -> Level {
        toml::from_str(REFERENCE_LEVEL).expect("reference level parses")
    }

    #[test]
    fn shipped_level_is_valid() {
        let level: Level = toml::from_str(SHIPPED_LEVEL).expect("shipped level parses");
        assert_eq!(level.validate(), Ok(()));
    }

    #[test]
    fn reference_level_is_valid() {
        let level = reference_level();
        assert_eq!(level.validate(), Ok(()));
    }

    #[test]
    fn dangling_connection_is_rejected() {
        let mut level = reference_level();
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
        let mut level = reference_level();
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

    fn route_mut(level: &mut Level, id: RouteId) -> &mut RouteData {
        level
            .stations
            .iter_mut()
            .flat_map(|s| &mut s.routes)
            .find(|r| r.id == id)
            .expect("route exists")
    }

    #[test]
    fn shared_block_is_rejected() {
        let mut level = reference_level();
        level.sections.push(SectionData {
            id: 99,
            blocks: vec![4],
        });
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::SharedBlock {
                block: 4,
                first: 2,
                second: 99
            })
        );
    }

    #[test]
    fn disconnected_section_is_rejected() {
        let mut level = reference_level();
        level.sections.push(SectionData {
            id: 99,
            blocks: vec![1, 11],
        });
        assert_eq!(level.validate(), Err(LevelValidationError::DisconnectedSection(99)));
    }

    #[test]
    fn route_paths_follow_the_switch_positions() {
        let level = reference_level();
        let path = |id| {
            let route = level.stations.iter().flat_map(|s| &s.routes).find(|r| r.id == id);
            level.route_path(route.expect("route exists")).expect("valid route")
        };
        assert_eq!(path(1), [3, 4, 5, 6], "facing, straight");
        assert_eq!(path(2), [3, 4, 20, 21, 22], "facing, side");
        assert_eq!(path(8), [21, 20, 4, 3, 2], "trailing, side");
    }

    #[test]
    fn route_without_its_switch_is_rejected() {
        let mut level = reference_level();
        route_mut(&mut level, 1).switches.clear();
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::RoutePath {
                route: 1,
                error: RoutePathError::UnsetSwitch(1)
            })
        );
    }

    #[test]
    fn route_trailing_a_switch_set_the_other_way_is_rejected() {
        let mut level = reference_level();
        route_mut(&mut level, 8).switches[0].position = SwitchPosition::Straight;
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::RoutePath {
                route: 8,
                error: RoutePathError::SwitchSetAgainst(1)
            })
        );
    }

    #[test]
    fn route_leaving_its_sections_is_rejected() {
        let mut level = reference_level();
        route_mut(&mut level, 1).target = 10;
        assert_eq!(
            level.validate(),
            Err(LevelValidationError::RoutePath {
                route: 1,
                error: RoutePathError::LeavesSections(6)
            })
        );
    }
}

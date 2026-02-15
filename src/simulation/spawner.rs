use crate::assets::{AssetHandles, LoadingState};
use crate::common::{BlockId, Direction, TrainId};
use crate::level::{Level, SpawnerKind};
use crate::simulation::block::{BlockMap, TrackPoint};
use crate::simulation::messages::{BlockUpdate, BlockUpdateState};
use crate::simulation::signal::{SignalAspect, SpeedControl};
use crate::simulation::train::{RailVehicle, TrainDespawnRequest, TrainSpawnRequest, get_random_train_number};
use bevy::prelude::*;
use std::collections::HashMap;

const SPAWNER_BLOCK_LENGTH: f64 = 2000.0;
const SPAWNER_SIGNAL_OFFSET: f64 = 5.0;
const SPAWNER_POINT_OFFSET: f64 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnTrainType {
    Cargo,
    Passenger,
    Locomotive,
}

#[derive(Event)]
pub struct SpawnRequest {
    pub block_id: BlockId,
    pub train_type: SpawnTrainType,
}

#[derive(Component)]
struct Spawner {
    block_id: BlockId,
    approach_block_id: BlockId,
    kind: SpawnerKind,
    direction: Direction,
    train: Option<TrainId>,
}

impl Spawner {
    fn get_spawn_point(&self) -> TrackPoint {
        let offset = match self.direction {
            Direction::Even => SPAWNER_BLOCK_LENGTH - SPAWNER_POINT_OFFSET,
            Direction::Odd => SPAWNER_POINT_OFFSET,
        };
        TrackPoint::new(self.block_id, offset)
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
struct SpawnerMapper(HashMap<BlockId, Entity>);

pub struct SpawnerPlugin;

impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnerMapper>()
            .add_observer(spawn_requests)
            .add_systems(OnEnter(LoadingState::Instantiated), init)
            .add_systems(Update, update.run_if(in_state(LoadingState::Instantiated)));
    }
}

fn init(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    mut block_map: ResMut<BlockMap>,
    mut block_updates: MessageWriter<BlockUpdate>,
    mut spawner_mapper: ResMut<SpawnerMapper>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    for spawner in &level.spawners {
        let (block_id, direction) = block_map.add_block(SPAWNER_BLOCK_LENGTH, spawner.block_id);
        block_map.add_signal(
            TrackPoint::new(block_id, SPAWNER_SIGNAL_OFFSET),
            Direction::Odd,
            SpeedControl::default_for_aspect(SignalAspect::Unrestricting),
            format!("SpawnerEndOdd{}", spawner.block_id),
        );
        block_map.add_signal(
            TrackPoint::new(block_id, SPAWNER_BLOCK_LENGTH - SPAWNER_SIGNAL_OFFSET),
            Direction::Even,
            SpeedControl::default_for_aspect(SignalAspect::Unrestricting),
            format!("SpawnerEndEven{}", spawner.block_id),
        );

        let entity = commands
            .spawn(Spawner {
                block_id,
                approach_block_id: spawner.block_id,
                kind: spawner.kind,
                direction: direction.reverse(),
                train: None,
            })
            .id();
        // Insert for both IDs, so that we can query the relevant spawner for events in either block
        spawner_mapper.insert(spawner.block_id, entity);
        spawner_mapper.insert(block_id, entity);
        block_updates.write(BlockUpdate::freed(block_id, 0));
    }
}

fn update(
    spawner_mapper: Res<SpawnerMapper>,
    mut query: Query<&mut Spawner>,
    mut block_updates: MessageReader<BlockUpdate>,
    mut despawn_requests: MessageWriter<TrainDespawnRequest>,
) {
    for update in block_updates.read() {
        if let Some(entity) = spawner_mapper.get(&update.block_id) {
            let mut spawner = query.get_mut(*entity).expect("invalid spawner entity");
            match update.state {
                BlockUpdateState::Occupied => {
                    if update.block_id == spawner.block_id {
                        let existing = spawner.train.replace(update.train_id);
                        if let Some(existing) = existing {
                            warn!(
                                "Spawner {} was already occupied by train {}",
                                spawner.approach_block_id, existing
                            );
                        }
                    }
                }
                BlockUpdateState::Freed => {
                    if update.block_id == spawner.block_id {
                        spawner.train = None;
                    } else if update.block_id == spawner.approach_block_id
                        && spawner.kind != SpawnerKind::Spawn
                        && spawner.train == Some(update.train_id)
                    {
                        spawner.train = None;
                        despawn_requests.write(TrainDespawnRequest { id: update.train_id });
                    }
                }
            }
        }
    }
}

fn spawn_requests(
    request: On<SpawnRequest>,
    spawner_mapper: Res<SpawnerMapper>,
    query: Query<&Spawner>,
    mut spawn_requests: MessageWriter<TrainSpawnRequest>,
) {
    if let Some(entity) = spawner_mapper.get(&request.block_id) {
        let spawner = query.get(*entity).expect("invalid spawner entity");

        let mut vehicles = Vec::new();
        match request.train_type {
            SpawnTrainType::Cargo => {
                vehicles.extend([RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0); 2]);
                vehicles.extend([RailVehicle::new_car(24_000.0, 15.0, 70_000.0); 60]);
            }
            SpawnTrainType::Passenger => {
                vehicles.push(RailVehicle::new_locomotive(80_000.0, 16.0, 2942.0, 300.0));
                vehicles.extend([RailVehicle::new_car(40_000.0, 24.0, 5_000.0); 25]);
            }
            SpawnTrainType::Locomotive => {
                vehicles.push(RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0));
                vehicles.push(RailVehicle::new_locomotive(138_000.0, 18.15, 2250.0, 375.0));
            }
        }

        spawn_requests.write(TrainSpawnRequest {
            number: get_random_train_number(),
            top_speed_kmh: 80.0,
            actual_speed_kmh: 75.0,
            position: spawner.get_spawn_point(),
            direction: spawner.direction,
            vehicles,
        });
    }
}

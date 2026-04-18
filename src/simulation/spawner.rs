use crate::assets::{AssetHandles, LoadingState};
use crate::audio::AudioEvent;
use crate::common::{BlockId, Direction, TrainId};
use crate::level::{Level, SpawnerKind};
use crate::simulation::block::{BlockMap, TrackPoint};
use crate::simulation::messages::{BlockUpdate, BlockUpdateState, SignalUpdate, SignalUpdateState};
use crate::simulation::signal::SignalAspect;
use crate::simulation::train::{RailVehicle, TrainDespawnRequest, TrainSpawnRequest, get_random_train_number};
use bevy::prelude::*;
use std::collections::HashMap;

const SPAWNER_POINT_OFFSET: f64 = 400.0;

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

struct Occupation {
    train_id: TrainId,
    num_blocks: u8,
}

impl Occupation {
    fn new(train_id: TrainId) -> Self {
        Occupation {
            train_id,
            num_blocks: 1,
        }
    }

    fn occupy(&mut self, train_id: TrainId) -> Result<u8, ()> {
        if train_id == self.train_id {
            self.num_blocks += 1;
            Ok(self.num_blocks)
        } else {
            Err(())
        }
    }

    fn free(&mut self, train_id: TrainId) -> Result<u8, ()> {
        if train_id == self.train_id {
            self.num_blocks -= 1;
            Ok(self.num_blocks)
        } else {
            Err(())
        }
    }
}

#[derive(Component)]
struct Spawner {
    block_id: BlockId,
    direction: Direction,
    speed_kmh: f64,
    spawn_point: TrackPoint,
    train: Option<Occupation>,
}

impl Spawner {
    fn is_busy(&self) -> bool {
        self.train.is_some()
    }
}

#[derive(Component)]
struct Despawner {
    block_id: BlockId,
    adjacent_block_id: BlockId,
    train: Option<TrainId>,
}

#[derive(Resource, Deref, DerefMut, Default)]
struct SpawnerMapper(HashMap<BlockId, Entity>);

pub struct SpawnerPlugin;

impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnerMapper>()
            .add_observer(spawn_requests)
            .add_systems(OnEnter(LoadingState::Instantiated), init)
            .add_systems(
                Update,
                (update_spawners, update_despawners).run_if(in_state(LoadingState::Instantiated)),
            );
    }
}

fn init(
    handles: Res<AssetHandles>,
    levels: Res<Assets<Level>>,
    block_map: Res<BlockMap>,
    mut signal_updates: MessageWriter<SignalUpdate>,
    mut spawn_mapper: ResMut<SpawnerMapper>,
    mut commands: Commands,
) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    for data in &level.spawners {
        let block = block_map.get_block(data.block_id).expect("invalid block ID");
        if let Some(end_direction) = block.get_end_direction() {
            let direction = end_direction.reverse();
            let mut entity = commands.spawn(());
            spawn_mapper.insert(block.id, entity.id());

            if matches!(data.kind, SpawnerKind::Spawn | SpawnerKind::Both) {
                let spawn_offset = match direction {
                    Direction::Even => block.length_m - SPAWNER_POINT_OFFSET,
                    Direction::Odd => SPAWNER_POINT_OFFSET,
                };
                entity.insert(Spawner {
                    block_id: block.id,
                    direction,
                    speed_kmh: data.speed_kmh,
                    spawn_point: TrackPoint::new(block.id, spawn_offset),
                    train: None,
                });
                // Add approach blocks so we can detect changes there as well
                if data.approach_len > 0 {
                    block_map
                        .walk(&block.middle(), f64::INFINITY, direction)
                        .take(data.approach_len as usize + 1)
                        .for_each(|point| {
                            spawn_mapper.insert(point.block_id, entity.id());
                        });
                }
            }

            if matches!(data.kind, SpawnerKind::Despawn | SpawnerKind::Both) {
                let adjacent_block = block_map.get_next(block.id, direction).expect("invalid block ID");
                entity.insert(Despawner {
                    block_id: block.id,
                    adjacent_block_id: adjacent_block.id,
                    train: None,
                });
                spawn_mapper.insert(adjacent_block.id, entity.id());
                // If the end signal is present, open it permanently
                if let Some(signal) = block_map.find_signal(block.id, end_direction) {
                    signal_updates.write(SignalUpdate::new(
                        signal.id,
                        SignalUpdateState::SignalPropagation(SignalAspect::Unrestricting),
                    ));
                }
            }
        } else {
            panic!("spawner block {} has no open end", block.id)
        }
    }
}

fn update_spawners(
    spawner_mapper: Res<SpawnerMapper>,
    mut query: Query<&mut Spawner>,
    mut block_updates: MessageReader<BlockUpdate>,
) {
    for update in block_updates.read() {
        if let Some(entity) = spawner_mapper.get(&update.block_id)
            && let Ok(mut spawner) = query.get_mut(*entity)
        {
            let spawner_id = spawner.block_id;
            match update.state {
                BlockUpdateState::Occupied => {
                    if let Some(existing) = spawner.train.as_mut() {
                        if existing.occupy(update.train_id).is_err() {
                            warn!(
                                "Spawner {} was already occupied by train {}",
                                spawner_id, existing.train_id
                            );
                        }
                    } else {
                        spawner.train = Some(Occupation::new(update.train_id));
                    }
                }
                BlockUpdateState::Freed => {
                    if let Some(existing) = spawner.train.as_mut() {
                        match existing.free(update.train_id) {
                            Ok(0) => spawner.train = None,
                            Ok(_) => {}
                            Err(_) => {
                                warn!(
                                    "Spawner {} was occupied by train {}, freed by train {}",
                                    spawner_id, existing.train_id, update.train_id
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn update_despawners(
    spawner_mapper: Res<SpawnerMapper>,
    mut query: Query<&mut Despawner>,
    mut block_updates: MessageReader<BlockUpdate>,
    mut despawn_requests: MessageWriter<TrainDespawnRequest>,
) {
    for update in block_updates.read() {
        if let Some(entity) = spawner_mapper.get(&update.block_id)
            && let Ok(mut despawner) = query.get_mut(*entity)
        {
            let adjacent_update = update.block_id == despawner.adjacent_block_id;
            match update.state {
                BlockUpdateState::Occupied => {
                    if adjacent_update && despawner.train.is_none() {
                        despawner.train = Some(update.train_id);
                    }
                }
                BlockUpdateState::Freed => {
                    if update.block_id == despawner.block_id {
                        despawner.train = None;
                    } else if adjacent_update && despawner.train == Some(update.train_id) {
                        despawner.train = None;
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
    mut commands: Commands,
) {
    if let Some(entity) = spawner_mapper.get(&request.block_id) {
        let spawner = query.get(*entity).expect("invalid spawner entity");

        if spawner.is_busy() {
            warn!("Spawner {} is currently occupied", spawner.block_id);
            commands.trigger(AudioEvent::error());
            return;
        }

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
            actual_speed_kmh: spawner.speed_kmh,
            position: spawner.spawn_point.clone(),
            direction: spawner.direction,
            vehicles,
        });
    }
}

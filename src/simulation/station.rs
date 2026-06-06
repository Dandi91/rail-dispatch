use crate::assets::{AssetHandles, LoadingState};
use crate::audio::AudioEvent;
use crate::common::{BlockId, Direction, RouteId, SectionId, SignalId, SwitchId, SwitchPosition};
use crate::level::{Level, SwitchData, SwitchSetting};
use crate::simulation::block::{SignalUpdate, SignalUpdateSource, TrackState, TrackUpdate};
use crate::simulation::signal::SignalAspect;
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::iter::once;

#[derive(Message)]
pub struct SwitchUpdate {
    pub switch_id: SwitchId,
    pub position: SwitchPosition,
}

impl SwitchUpdate {
    pub fn new(switch_id: SwitchId, position: SwitchPosition) -> Self {
        Self { switch_id, position }
    }
}

/// Marks a route's blocks as pending (set & locked) or no longer pending. The panel paints
/// pending blocks green; it consumes the green path itself as occupancy updates arrive, so
/// this only fires on route activation (`pending = true`) and deactivation (`false`).
#[derive(Message)]
pub struct RoutePending {
    pub blocks: Vec<BlockId>,
    pub pending: bool,
}

pub struct Switch {
    pub id: SwitchId,
    pub base: BlockId,
    pub straight: BlockId,
    pub side: BlockId,
    pub direction: Direction,
    pub position: SwitchPosition,
}

impl From<&SwitchData> for Switch {
    fn from(data: &SwitchData) -> Self {
        Self {
            id: data.id,
            base: data.base,
            straight: data.straight,
            side: data.side,
            direction: data.direction,
            position: SwitchPosition::Straight,
        }
    }
}

impl Chunkable for Switch {
    fn get_id(&self) -> u32 {
        self.id
    }
}

#[derive(Default)]
struct BusyTracker(HashSet<BlockId>);

impl BusyTracker {
    fn is_free(&self) -> bool {
        self.0.is_empty()
    }

    fn handle_update(&mut self, update: &TrackUpdate) {
        match update.state {
            TrackState::Occupied => {
                self.0.extend(update.blocks());
            }
            TrackState::Freed => {
                for block in update.blocks() {
                    self.0.remove(block);
                }
            }
        }
    }
}

#[derive(Default, PartialEq)]
enum RouteState {
    #[default]
    /// Route isn't set, signal closed
    Inactive,
    /// Route set, signal opened, awaiting train
    Active,
    /// Route set, train entered, signal closed
    Used,
}

#[derive(Default)]
struct Route {
    id: RouteId,
    signal_id: SignalId,
    block_ids: Vec<BlockId>,
    target_block_id: BlockId,
    switch_settings: Vec<SwitchSetting>,
    tracker: BusyTracker,
    state: RouteState,
    target_block_state: TrackState,
}

impl Chunkable for Route {
    fn get_id(&self) -> u32 {
        self.id
    }
}

impl Route {
    fn all_blocks(&self) -> impl Iterator<Item = BlockId> {
        self.block_ids.iter().copied().chain(once(self.target_block_id))
    }
}

#[derive(Resource)]
pub struct StationMap {
    routes: SparseVec<Route>,
    blocks_to_routes: HashMap<BlockId, Vec<RouteId>>,
    conflicting_routes: HashMap<RouteId, Vec<RouteId>>,
}

impl StationMap {
    pub fn from_level(level: &Level) -> Self {
        let sections: HashMap<SectionId, &Vec<BlockId>> = level.sections.iter().map(|sd| (sd.id, &sd.blocks)).collect();

        let routes: SparseVec<Route> = level
            .stations
            .iter()
            .flat_map(|sd| sd.routes.iter())
            .map(|rd| {
                let block_ids: Vec<BlockId> = rd
                    .sections
                    .iter()
                    .flat_map(|sid| sections[sid].iter().copied())
                    .collect();
                Route {
                    id: rd.id,
                    signal_id: rd.signal,
                    block_ids,
                    target_block_id: rd.target,
                    switch_settings: rd.switches.clone(),
                    ..Default::default()
                }
            })
            .collect();

        let mut blocks_to_routes: HashMap<BlockId, Vec<RouteId>> = HashMap::new();
        for route in &routes {
            blocks_to_routes
                .entry(route.target_block_id)
                .or_default()
                .push(route.id);
            for &block_id in &route.block_ids {
                blocks_to_routes.entry(block_id).or_default().push(route.id);
            }
        }

        let route_blocks: HashMap<RouteId, HashSet<BlockId>> =
            routes.iter().map(|r| (r.id, r.all_blocks().collect())).collect();

        let conflicting_routes: HashMap<RouteId, Vec<RouteId>> = route_blocks
            .iter()
            .map(|(&rid, blocks)| {
                let conflicts = route_blocks
                    .iter()
                    .filter(|&(&other, other_blocks)| other != rid && !blocks.is_disjoint(other_blocks))
                    .map(|(&other, _)| other)
                    .collect();
                (rid, conflicts)
            })
            .collect();

        StationMap {
            routes,
            blocks_to_routes,
            conflicting_routes,
        }
    }

    fn track_route_state(&mut self, track_updates: &mut MessageReader<TrackUpdate>) {
        let mut recheck_route_ids = HashSet::new();
        for update in track_updates.read() {
            if let Some(route_ids) = self.blocks_to_routes.get(&update.block_id) {
                for &route_id in route_ids {
                    let route = &mut self.routes[route_id];
                    if update.block_id == route.target_block_id {
                        route.target_block_state = update.state;
                    } else {
                        route.tracker.handle_update(update);
                        recheck_route_ids.insert(route_id);
                    }
                }
            }
        }

        for route_id in recheck_route_ids {
            let route = &mut self.routes[route_id];
            match route.state {
                RouteState::Active if !route.tracker.is_free() => route.state = RouteState::Used,
                RouteState::Used if route.tracker.is_free() => {
                    route.state = RouteState::Inactive;
                }
                _ => {}
            };
        }
    }

    fn handle_route_activation(
        &mut self,
        requests: &mut MessageReader<RouteActivationRequest>,
        signal_updates: &mut MessageWriter<SignalUpdate>,
        switch_updates: &mut MessageWriter<SwitchUpdate>,
        route_pending: &mut MessageWriter<RoutePending>,
        commands: &mut Commands,
    ) {
        for req in requests.read() {
            let route = &self.routes[req.route_id];
            if route.state != RouteState::Inactive {
                warn!("Route {} is already active", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            if !route.tracker.is_free() {
                warn!("Route {} sections are occupied", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            let conflict = self
                .conflicting_routes
                .get(&req.route_id)
                .is_some_and(|v| v.iter().any(|&rid| self.routes[rid].state != RouteState::Inactive));
            if conflict {
                warn!("Route {} conflicts with other routes", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            if route.target_block_state == TrackState::Occupied {
                warn!("Route {} target block is occupied", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            switch_updates.write_batch(
                route
                    .switch_settings
                    .iter()
                    .map(|s| SwitchUpdate::new(s.switch_id, s.position)),
            );

            signal_updates.write(SignalUpdate::new(
                route.signal_id,
                SignalUpdateSource::Manual(SignalAspect::Unrestricting),
            ));

            let route = &mut self.routes[req.route_id];
            route.state = RouteState::Active;
            route_pending.write(RoutePending {
                blocks: route.all_blocks().collect(),
                pending: true,
            });
            commands.trigger(AudioEvent::beep());
        }
    }
}

#[derive(Message)]
pub struct RouteActivationRequest {
    pub route_id: RouteId,
}

pub struct StationPlugin;

impl Plugin for StationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(LoadingState::Instantiated), build_station_map)
            .add_systems(
                Update,
                (track_route_state, handle_route_activation).run_if(in_state(LoadingState::Instantiated)),
            )
            .add_message::<RouteActivationRequest>()
            .add_message::<RoutePending>()
            .add_message::<SwitchUpdate>();
    }
}

fn build_station_map(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    commands.insert_resource(StationMap::from_level(level));
}

fn track_route_state(mut station_map: ResMut<StationMap>, mut block_updates: MessageReader<TrackUpdate>) {
    station_map.track_route_state(&mut block_updates);
}

fn handle_route_activation(
    mut station_map: ResMut<StationMap>,
    mut requests: MessageReader<RouteActivationRequest>,
    mut signal_updates: MessageWriter<SignalUpdate>,
    mut switch_updates: MessageWriter<SwitchUpdate>,
    mut route_pending: MessageWriter<RoutePending>,
    mut commands: Commands,
) {
    station_map.handle_route_activation(
        &mut requests,
        &mut signal_updates,
        &mut switch_updates,
        &mut route_pending,
        &mut commands,
    );
}

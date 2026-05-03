use crate::assets::{AssetHandles, LoadingState};
use crate::audio::AudioEvent;
use crate::common::{BlockId, Direction, LampId, RouteId, SectionId, SignalId, StationId, SwitchId, SwitchPosition};
use crate::level::{Level, SwitchData, SwitchSetting};
use crate::simulation::block::{BlockState, BlockUpdate, LampUpdate, SetPending, SignalUpdate, SignalUpdateState};
use crate::simulation::signal::SignalAspect;
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

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

    fn handle_update(&mut self, update: &BlockUpdate) {
        match update.state {
            BlockState::Occupied => {
                self.0.insert(update.block_id);
            }
            BlockState::Freed => {
                self.0.remove(&update.block_id);
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
    section_ids: Vec<SectionId>,
    block_ids: Vec<BlockId>,
    target_block_id: BlockId,
    switch_settings: Vec<SwitchSetting>,
    tracker: BusyTracker,
    state: RouteState,
    target_block_state: BlockState,
}

impl Chunkable for Route {
    fn get_id(&self) -> u32 {
        self.id
    }
}

impl Route {
    fn get_lamp_blocks(&self) -> Vec<BlockId> {
        let mut result = self.block_ids.clone();
        result.push(self.target_block_id);
        result
    }
}

#[derive(Default)]
struct Section {
    id: SectionId,
    blocks: Vec<BlockId>,
    lamps: Vec<LampId>,
    tracker: BusyTracker,
    /// True while at least one route claiming this section is in `Active` or `Used` state.
    /// Lamp updates fire only while this is true; otherwise the section is invisible.
    active: bool,
}

impl Chunkable for Section {
    fn get_id(&self) -> u32 {
        self.id
    }
}

struct Station {
    id: StationId,
    name: String,
}

impl Chunkable for Station {
    fn get_id(&self) -> u32 {
        self.id
    }
}

#[derive(Resource)]
struct StationMap {
    stations: SparseVec<Station>,
    routes: SparseVec<Route>,
    sections: SparseVec<Section>,
    blocks_to_routes: HashMap<BlockId, Vec<RouteId>>,
    blocks_to_sections: HashMap<BlockId, Vec<SectionId>>,
    conflicting_routes: HashMap<RouteId, Vec<RouteId>>,
}

impl StationMap {
    pub fn from_level(level: &Level) -> Self {
        let block_lamps: HashMap<BlockId, LampId> = level.blocks.iter().map(|bd| (bd.id, bd.lamp_id)).collect();

        let sections: SparseVec<Section> = level
            .sections
            .iter()
            .map(|sd| Section {
                id: sd.id,
                blocks: sd.blocks.clone(),
                lamps: sd.blocks.iter().map(|bid| block_lamps[bid]).collect(),
                ..Default::default()
            })
            .collect();

        let routes: SparseVec<Route> = level
            .stations
            .iter()
            .flat_map(|sd| sd.routes.iter())
            .map(|rd| {
                let block_ids: Vec<BlockId> = rd
                    .sections
                    .iter()
                    .flat_map(|&sid| sections[sid].blocks.iter().copied())
                    .collect();
                Route {
                    id: rd.id,
                    signal_id: rd.signal,
                    section_ids: rd.sections.clone(),
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

        let mut blocks_to_sections: HashMap<BlockId, Vec<SectionId>> = HashMap::new();
        for section in &sections {
            for &block_id in &section.blocks {
                blocks_to_sections.entry(block_id).or_default().push(section.id);
            }
        }

        let route_blocks: HashMap<RouteId, HashSet<BlockId>> = routes
            .iter()
            .map(|r| (r.id, r.block_ids.iter().copied().collect()))
            .collect();

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

        let stations = level
            .stations
            .iter()
            .map(|sd| Station {
                id: sd.id,
                name: sd.name.clone(),
            })
            .collect();

        StationMap {
            stations,
            routes,
            sections,
            blocks_to_routes,
            blocks_to_sections,
            conflicting_routes,
        }
    }

    fn track_route_state(
        &mut self,
        block_updates: &mut MessageReader<BlockUpdate>,
        lamp_updates: &mut MessageWriter<LampUpdate>,
    ) {
        let mut recheck_route_ids = HashSet::new();
        for update in block_updates.read() {
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
            if let Some(section_ids) = self.blocks_to_sections.get(&update.block_id) {
                for &section_id in section_ids {
                    let section = &mut self.sections[section_id];
                    let was_free = section.tracker.is_free();
                    section.tracker.handle_update(update);
                    if section.active && was_free != section.tracker.is_free() {
                        lamp_updates.write_batch(
                            section
                                .lamps
                                .iter()
                                .map(|&lamp_id| LampUpdate::from_block_state(update.state, lamp_id)),
                        );
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
                    for &section_id in &self.routes[route_id].section_ids {
                        self.sections[section_id].active = false;
                    }
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

            if route.target_block_state == BlockState::Occupied {
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
                SignalUpdateState::Manual(SignalAspect::Unrestricting),
            ));

            for &section_id in &route.section_ids {
                self.sections[section_id].active = true;
            }

            let route = &mut self.routes[req.route_id];
            route.state = RouteState::Active;
            commands.trigger(SetPending(route.get_lamp_blocks()));
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
            .add_message::<SwitchUpdate>();
    }
}

fn build_station_map(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    commands.insert_resource(StationMap::from_level(level));
}

fn track_route_state(
    mut station_map: ResMut<StationMap>,
    mut block_updates: MessageReader<BlockUpdate>,
    mut lamp_updates: MessageWriter<LampUpdate>,
) {
    station_map.track_route_state(&mut block_updates, &mut lamp_updates);
}

fn handle_route_activation(
    mut station_map: ResMut<StationMap>,
    mut requests: MessageReader<RouteActivationRequest>,
    mut signal_updates: MessageWriter<SignalUpdate>,
    mut switch_updates: MessageWriter<SwitchUpdate>,
    mut commands: Commands,
) {
    station_map.handle_route_activation(&mut requests, &mut signal_updates, &mut switch_updates, &mut commands);
}

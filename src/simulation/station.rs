use crate::assets::{AssetHandles, LoadingState};
use crate::audio::AudioEvent;
use crate::common::{BlockId, Direction, RouteId, SectionId, SignalId, StationId, SwitchId, SwitchPosition};
use crate::level::{Level, SwitchData, SwitchSetting};
use crate::simulation::block::{BlockMap, BlockUpdate, BlockUpdateState, LampUpdate, SignalUpdate, SignalUpdateState};
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

struct Section {
    id: SectionId,
    blocks: Vec<BlockId>,
    occupied: HashSet<BlockId>,
}

impl Section {
    fn is_free(&self) -> bool {
        self.occupied.is_empty()
    }
}

impl Chunkable for Section {
    fn get_id(&self) -> u32 {
        self.id
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

struct Route {
    id: RouteId,
    signal_id: SignalId,
    station_id: StationId,
    section_ids: Vec<SectionId>,
    switch_settings: Vec<SwitchSetting>,
    state: RouteState,
}

impl Chunkable for Route {
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
    block_to_sections: HashMap<BlockId, Vec<SectionId>>,
    conflicting_routes: HashMap<RouteId, Vec<RouteId>>,
}

impl StationMap {
    pub fn from_level(level: &Level) -> Self {
        let sections: SparseVec<Section> = level
            .sections
            .iter()
            .map(|sd| Section {
                id: sd.id,
                blocks: sd.blocks.clone(),
                occupied: HashSet::new(),
            })
            .collect();

        let mut block_to_sections: HashMap<BlockId, Vec<SectionId>> = HashMap::new();
        for section in sections.iter() {
            for &block_id in &section.blocks {
                block_to_sections.entry(block_id).or_default().push(section.id);
            }
        }

        let routes: SparseVec<Route> = level
            .stations
            .iter()
            .flat_map(|sd| sd.routes.iter().zip(std::iter::repeat(sd.id)))
            .map(|(rd, s_id)| Route {
                id: rd.id,
                signal_id: rd.signal,
                station_id: s_id,
                section_ids: rd.sections.clone(),
                switch_settings: rd.switches.clone(),
                state: RouteState::Inactive,
            })
            .collect();

        let route_blocks: HashMap<RouteId, HashSet<BlockId>> = routes
            .iter()
            .map(|r| {
                let blocks = r
                    .section_ids
                    .iter()
                    .flat_map(|&sid| sections.get(sid).expect("invalid section id").blocks.iter().copied())
                    .collect();
                (r.id, blocks)
            })
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
            block_to_sections,
            conflicting_routes,
        }
    }

    fn track_section_occupancy(&mut self, block_updates: &mut MessageReader<BlockUpdate>) {
        for update in block_updates.read() {
            if let Some(section_ids) = self.block_to_sections.get(&update.block_id) {
                for &section_id in section_ids {
                    let section = self.sections.get_mut(section_id).expect("invalid section id");
                    match update.state {
                        BlockUpdateState::Occupied => {
                            section.occupied.insert(update.block_id);
                        }
                        BlockUpdateState::Freed => {
                            section.occupied.remove(&update.block_id);
                        }
                    }
                }
            }
        }

        let mut route_updates = Vec::new();
        for route in &self.routes {
            let any_occupied = self.is_route_free(route.id);
            match route.state {
                RouteState::Active if any_occupied => route_updates.push((route.id, RouteState::Used)),
                RouteState::Used if !any_occupied => route_updates.push((route.id, RouteState::Inactive)),
                _ => {}
            }
        }
        for (route_id, state) in route_updates {
            let route = self.routes.get_mut(route_id).expect("invalid route index");
            route.state = state;
        }
    }

    fn is_route_free(&self, route_id: RouteId) -> bool {
        let route = self.routes.get(route_id).expect("invalid route index");
        route
            .section_ids
            .iter()
            .all(|&sid| self.sections.get(sid).is_some_and(|s| s.is_free()))
    }

    fn handle_route_activation(
        &mut self,
        block_map: &BlockMap,
        requests: &mut MessageReader<RouteActivationRequest>,
        signal_updates: &mut MessageWriter<SignalUpdate>,
        switch_updates: &mut MessageWriter<SwitchUpdate>,
        lamp_updates: &mut MessageWriter<LampUpdate>,
        commands: &mut Commands,
    ) {
        for req in requests.read() {
            let route = self.routes.get(req.route_id).expect("invalid route index");
            if route.state != RouteState::Inactive {
                warn!("Route {} is already active", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            if !self.is_route_free(req.route_id) {
                warn!("Route {} sections are occupied", req.route_id);
                commands.trigger(AudioEvent::error());
                continue;
            }

            let conflict = self.conflicting_routes.get(&req.route_id).is_some_and(|v| {
                v.iter().any(|&rid| {
                    let route = self.routes.get(rid).expect("invalid route id");
                    route.state != RouteState::Inactive
                })
            });
            if conflict {
                warn!("Route {} conflicts with other routes", req.route_id);
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

            let route = self.routes.get_mut(req.route_id).expect("invalid route index");
            route.state = RouteState::Active;
            route.section_ids.iter().for_each(|&sid| {
                let section = self.sections.get_mut(sid).expect("invalid section id");
                section.blocks.iter().for_each(|&block_id| {
                    let block = block_map.get_block(block_id).expect("invalid block id");
                    lamp_updates.write(LampUpdate::pending(block.lamp_id));
                });
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
                (track_section_occupancy, handle_route_activation).run_if(in_state(LoadingState::Instantiated)),
            )
            .add_message::<RouteActivationRequest>()
            .add_message::<SwitchUpdate>();
    }
}

fn build_station_map(handles: Res<AssetHandles>, levels: Res<Assets<Level>>, mut commands: Commands) {
    let level = levels.get(&handles.level).expect("level had been loaded");
    commands.insert_resource(StationMap::from_level(level));
}

fn track_section_occupancy(mut station_map: ResMut<StationMap>, mut block_updates: MessageReader<BlockUpdate>) {
    station_map.track_section_occupancy(&mut block_updates);
}

fn handle_route_activation(
    block_map: Res<BlockMap>,
    mut station_map: ResMut<StationMap>,
    mut requests: MessageReader<RouteActivationRequest>,
    mut signal_updates: MessageWriter<SignalUpdate>,
    mut switch_updates: MessageWriter<SwitchUpdate>,
    mut lamp_updates: MessageWriter<LampUpdate>,
    mut commands: Commands,
) {
    station_map.handle_route_activation(
        &block_map,
        &mut requests,
        &mut signal_updates,
        &mut switch_updates,
        &mut lamp_updates,
        &mut commands,
    );
}

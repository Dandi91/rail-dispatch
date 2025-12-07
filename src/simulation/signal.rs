use crate::common::{BlockId, Direction, LampId, SignalId};
use crate::level::SignalData;
use crate::simulation::block::TrackPoint;
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use itertools::Itertools;
use std::collections::HashMap;

#[derive(Default)]
pub struct SignalMap {
    signals: SparseVec<TrackSignal>,
    map: HashMap<(BlockId, Direction), SignalId>,
}

impl SignalMap {
    #[inline]
    pub fn get(&self, id: SignalId) -> Option<&TrackSignal> {
        self.signals.get(id)
    }

    pub fn find_signal(&self, block_id: BlockId, direction: Direction) -> Option<&TrackSignal> {
        let signal_id = self.map.get(&(block_id, direction))?;
        self.signals.get(*signal_id)
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, TrackSignal> {
        self.signals.iter()
    }
}

impl FromIterator<TrackSignal> for SignalMap {
    fn from_iter<I: IntoIterator<Item = TrackSignal>>(iter: I) -> Self {
        let signals: SparseVec<TrackSignal> = iter.into_iter().map_into().collect();
        let map: HashMap<(BlockId, Direction), SignalId> = signals
            .iter()
            .map(|x| ((x.position.block_id, x.direction), x.id))
            .collect();

        Self { signals, map }
    }
}

pub enum SignalAspect {
    /// Signal does not restrict the train's speed
    Unrestricting,
    /// Signal restricts the train's speed to the allowed value
    Restricting(f64),
    /// Signal forbids the train from moving past it
    Forbidding,
}

#[derive(Default)]
pub struct SpeedControl {
    pub allowed_kmh: f64,
}

#[derive(Default)]
pub struct TrackSignal {
    pub id: SignalId,
    pub position: TrackPoint,
    pub lamp_id: LampId,
    pub direction: Direction,
    pub name: String,
    pub speed_ctrl: SpeedControl,
}

impl From<&SignalData> for TrackSignal {
    fn from(value: &SignalData) -> Self {
        TrackSignal {
            id: value.id,
            position: TrackPoint {
                block_id: value.block_id,
                offset_m: value.offset_m,
            },
            lamp_id: value.lamp_id,
            direction: value.direction,
            name: value.name.clone(),
            speed_ctrl: SpeedControl { allowed_kmh: 80.0 },
            ..Default::default()
        }
    }
}

impl Chunkable for TrackSignal {
    #[inline]
    fn get_id(&self) -> u32 {
        self.id
    }
}

impl TrackSignal {
    #[inline]
    pub fn get_allowed_speed_mps(&self) -> f64 {
        self.speed_ctrl.allowed_kmh / 3.6
    }

    #[inline]
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn get_lamp_id(&self) -> LampId {
        self.lamp_id
    }
}

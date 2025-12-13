use crate::common::{BlockId, Direction, LampId, SignalId};
use crate::level::SignalData;
use crate::simulation::block::TrackPoint;
use crate::simulation::sparse_vec::{Chunkable, SparseVec};
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt::Display;

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

    #[inline]
    pub fn get_mut(&mut self, id: SignalId) -> Option<&mut TrackSignal> {
        self.signals.get_mut(id)
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

#[derive(Default, Copy, Clone)]
pub enum SpeedLimit {
    #[default]
    Unrestricted,
    Restricted(f64),
}

impl Display for SpeedLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeedLimit::Unrestricted => write!(f, "unrestricted"),
            SpeedLimit::Restricted(speed) => write!(f, "{:.0} km/h", speed),
        }
    }
}

impl SpeedLimit {
    pub fn to_mps(&self, unrestricted_kmh: f64) -> f64 {
        match self {
            SpeedLimit::Unrestricted => unrestricted_kmh / 3.6,
            SpeedLimit::Restricted(speed_kmh) => speed_kmh / 3.6,
        }
    }
}

#[derive(Default, Copy, Clone, PartialEq)]
pub enum SignalAspect {
    /// Signal does not restrict the train's speed
    #[default]
    Unrestricting,
    /// Signal restricts the train's speed to the allowed value
    Restricting,
    /// Signal forbids the train from moving past it
    Forbidding,
}

impl SignalAspect {
    pub fn chain(&self) -> SignalAspect {
        match self {
            SignalAspect::Unrestricting | SignalAspect::Restricting => SignalAspect::Unrestricting,
            SignalAspect::Forbidding => SignalAspect::Restricting,
        }
    }
}

#[derive(Default)]
pub struct SpeedControl {
    pub aspect: SignalAspect,
    pub passing_kmh: SpeedLimit,
    pub approaching_kmh: SpeedLimit,
}

impl SpeedControl {
    pub fn chain(&self) -> SpeedControl {
        Self::default_for_aspect(self.aspect.chain())
    }

    pub fn default_for_aspect(aspect: SignalAspect) -> SpeedControl {
        match aspect {
            SignalAspect::Unrestricting => SpeedControl::default(),
            SignalAspect::Restricting => SpeedControl {
                aspect,
                passing_kmh: SpeedLimit::Restricted(40.0),
                approaching_kmh: SpeedLimit::Unrestricted,
            },
            SignalAspect::Forbidding => SpeedControl {
                aspect,
                passing_kmh: SpeedLimit::Restricted(0.0),
                approaching_kmh: SpeedLimit::Restricted(40.0),
            },
        }
    }
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
    pub fn change_aspect(&mut self, aspect: SignalAspect) {
        self.speed_ctrl = SpeedControl::default_for_aspect(aspect);
    }
}

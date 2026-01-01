use crate::common::{BlockId, Direction, SwitchId};
use crate::level::SwitchData;
use crate::simulation::sparse_vec::Chunkable;

pub struct Switch {
    pub id: SwitchId,
    pub base: BlockId,
    pub straight: BlockId,
    pub side: BlockId,
    pub direction: Direction,
}

impl From<&SwitchData> for Switch {
    fn from(data: &SwitchData) -> Self {
        Self {
            id: data.id,
            base: data.base,
            straight: data.straight,
            side: data.side,
            direction: data.direction,
        }
    }
}

impl Chunkable for Switch {
    fn get_id(&self) -> u32 {
        self.id
    }
}

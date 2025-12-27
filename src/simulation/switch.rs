use crate::common::{BlockId, SwitchId};
use crate::simulation::sparse_vec::Chunkable;

pub struct Switch {
    id: SwitchId,
    base: BlockId,
    straight: BlockId,
    side: BlockId,
}

impl Switch {
    pub fn new(id: SwitchId, base: BlockId, straight: BlockId, side: BlockId) -> Self {
        Self {
            id,
            base,
            straight,
            side,
        }
    }
}

impl Chunkable for Switch {
    fn get_id(&self) -> u32 {
        self.id
    }
}

use crate::common::SignalId;
use crate::display::lamp::default_lamp_height;
use crate::{common::BlockId, common::Direction, common::LampId};
use bevy::{asset::AssetLoader, asset::LoadContext, asset::io::Reader, prelude::*};
use futures_lite::AsyncReadExt;
use serde::Deserialize;
use thiserror::Error;

#[derive(Deserialize, Asset, Reflect)]
pub struct Level {
    pub lamps: Vec<LampData>,
    pub blocks: Vec<BlockData>,
    pub connections: Vec<ConnectionData>,
    pub signals: Vec<SignalData>,
    #[serde(deserialize_with = "crate::common::deserialize_color")]
    pub background: Color,
}

#[derive(Deserialize, Reflect)]
pub struct LampData {
    pub id: LampId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    #[serde(default = "default_lamp_height")]
    pub height: f32,
}

#[derive(Deserialize, Reflect, Default)]
pub struct BlockData {
    pub id: BlockId,
    pub length: f64,
    pub lamp_id: LampId,
}

#[derive(Deserialize, Reflect)]
pub struct ConnectionData {
    pub start: BlockId,
    pub end: BlockId,
}

#[derive(Deserialize, Reflect, Clone)]
pub struct SignalData {
    pub id: SignalId,
    pub lamp_id: LampId,
    pub block_id: BlockId,
    pub offset_m: f64,
    pub name: String,
    pub direction: Direction,
}

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Level>().register_asset_loader(LevelLoader);
    }
}

struct LevelLoader;

#[derive(Debug, Error)]
enum LevelLoaderError {
    #[error("Failed to load level file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse level file: {0}")]
    FileTexture(#[from] toml::de::Error),
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
        Ok(toml::from_str(&contents)?)
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
}

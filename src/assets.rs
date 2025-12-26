use crate::level::Level;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use event_listener::Event;
use futures_lite::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Debug, Resource, Deref)]
struct AssetBarrier(Arc<AssetBarrierInner>);

#[derive(Debug, Deref)]
struct AssetBarrierGuard(Arc<AssetBarrierInner>);

#[derive(Debug, Resource)]
struct AssetBarrierInner {
    count: AtomicU32,
    notify: Event,
}

#[derive(Debug, Resource)]
struct AsyncLoadingState(Arc<AtomicBool>);

impl AssetBarrier {
    fn new() -> (AssetBarrier, AssetBarrierGuard) {
        let inner = Arc::new(AssetBarrierInner {
            count: AtomicU32::new(1),
            notify: Event::new(),
        });
        (AssetBarrier(inner.clone()), AssetBarrierGuard(inner))
    }

    fn wait_async(&self) -> impl Future<Output = ()> + 'static {
        let shared = self.0.clone();
        async move {
            loop {
                let listener = shared.notify.listen();
                if shared.count.load(Ordering::Acquire) == 0 {
                    return;
                }
                listener.await;
            }
        }
    }
}

impl Clone for AssetBarrierGuard {
    fn clone(&self) -> Self {
        self.count.fetch_add(1, Ordering::AcqRel);
        AssetBarrierGuard(self.0.clone())
    }
}

impl Drop for AssetBarrierGuard {
    fn drop(&mut self) {
        let prev = self.count.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            self.notify.notify(usize::MAX);
        }
    }
}
#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum LoadingState {
    #[default]
    Loading,
    Loaded,
}

#[derive(Resource)]
pub struct AssetHandles {
    pub level: Handle<Level>,
    pub board: Handle<Image>,
}

pub struct AssetLoadingPlugin;

impl Plugin for AssetLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingState>()
            .add_systems(Startup, setup_assets)
            .add_systems(Update, get_async_loading_state.run_if(in_state(LoadingState::Loading)));
    }
}

fn setup_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let (barrier, guard) = AssetBarrier::new();
    commands.insert_resource(AssetHandles {
        level: asset_server.load_acquire("level.toml", guard.clone()),
        board: asset_server.load_acquire("board.png", guard.clone()),
    });

    let future = barrier.wait_async();
    commands.insert_resource(barrier);

    let loading_state = Arc::new(AtomicBool::new(false));
    commands.insert_resource(AsyncLoadingState(loading_state.clone()));

    AsyncComputeTaskPool::get()
        .spawn(async move {
            future.await;
            loading_state.store(true, Ordering::Release);
        })
        .detach();

    info!("Asset load started");
}

fn get_async_loading_state(state: Res<AsyncLoadingState>, mut next_loading_state: ResMut<NextState<LoadingState>>) {
    if state.0.load(Ordering::Acquire) {
        info!("Asset load complete");
        next_loading_state.set(LoadingState::Loaded);
    }
}

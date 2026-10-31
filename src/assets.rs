use crate::level::Level;
use bevy::asset::LoadState;
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
        (AssetBarrier(Arc::clone(&inner)), AssetBarrierGuard(inner))
    }

    fn wait_async(&self) -> impl Future<Output = ()> + 'static {
        let shared = Arc::clone(&self.0);
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
        AssetBarrierGuard(Arc::clone(&self.0))
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
    Instantiated,
}

#[derive(Resource, Debug)]
pub struct AssetHandles {
    pub level: Handle<Level>,
}

#[derive(Resource, Debug)]
pub struct FontHandles {
    /// Monospace font (`DejaVuSansMono.ttf`), used for the dispatcher describer labels.
    pub mono: Handle<Font>,
}

#[derive(Resource, Debug)]
pub struct SoundHandles {
    pub beep: Handle<AudioSource>,
    pub error: Handle<AudioSource>,
    pub message: Handle<AudioSource>,
    pub notification: Handle<AudioSource>,
}

#[derive(Debug)]
pub struct AssetLoadingPlugin;

impl Plugin for AssetLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingState>()
            .add_systems(Startup, setup_assets)
            .add_systems(Update, poll_loading_state.run_if(in_state(LoadingState::Loading)));
    }
}

fn setup_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let (barrier, guard) = AssetBarrier::new();
    commands.insert_resource(AssetHandles {
        level: asset_server.load_builder().with_guard(guard).load("level.toml"),
    });
    commands.insert_resource(FontHandles {
        mono: asset_server.load("fonts/DejaVuSansMono.ttf"),
    });
    commands.insert_resource(SoundHandles {
        beep: asset_server.load("sounds/beep.wav"),
        error: asset_server.load("sounds/error.wav"),
        message: asset_server.load("sounds/message.wav"),
        notification: asset_server.load("sounds/notification.wav"),
    });

    let future = barrier.wait_async();
    commands.insert_resource(barrier);

    let loading_state = Arc::new(AtomicBool::new(false));
    commands.insert_resource(AsyncLoadingState(Arc::clone(&loading_state)));

    AsyncComputeTaskPool::get()
        .spawn(async move {
            future.await;
            loading_state.store(true, Ordering::Release);
        })
        .detach();

    info!("Asset load started");
}

fn poll_loading_state(
    state: Res<AsyncLoadingState>,
    handles: Res<AssetHandles>,
    asset_server: Res<AssetServer>,
    mut next_loading_state: ResMut<NextState<LoadingState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.0.load(Ordering::Acquire) {
        return;
    }
    // The barrier also releases when the load fails, so check the outcome before advancing:
    // every later stage assumes the level asset exists.
    match asset_server.load_state(&handles.level) {
        LoadState::Loaded => {
            info!("Asset load complete");
            next_loading_state.set(LoadingState::Loaded);
        }
        LoadState::Failed(err) => {
            error!("Failed to load level: {err}");
            exit.write(AppExit::error());
        }
        // The asset server hasn't processed the completion event yet; check next frame.
        LoadState::NotLoaded | LoadState::Loading => {}
    }
}

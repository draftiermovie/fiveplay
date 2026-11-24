//! The game's assets, compiled into the executable by `build.rs`, served to
//! Bevy as the default asset source. Players only need the `.exe`.
//!
//! Set `FIVEPLAY_ASSETS=disk` to load from the `assets/` folder instead
//! (handy while editing assets without rebuilding).
use bevy::{
    asset::io::{
        AssetSourceBuilder, AssetSourceId,
        memory::{Dir, MemoryAssetReader},
    },
    prelude::*,
};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

static FROM_DISK: AtomicBool = AtomicBool::new(false);

/// Loads from disk when `FIVEPLAY_ASSETS=disk`.
pub fn from_disk() -> bool {
    std::env::var("FIVEPLAY_ASSETS").is_ok_and(|v| v.eq_ignore_ascii_case("disk"))
}

/// Makes the embedded files the default asset source. Must run before
/// `DefaultPlugins` is added.
pub fn install(app: &mut App) {
    if from_disk() {
        FROM_DISK.store(true, Ordering::Relaxed);
        return;
    }
    let dir = Dir::default();
    for (path, bytes) in FILES {
        dir.insert_asset(Path::new(path), *bytes);
    }
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
}

/// Whether an asset path exists in the active source.
pub fn exists(path: &str) -> bool {
    if FROM_DISK.load(Ordering::Relaxed) {
        let root = std::env::var_os("BEVY_ASSET_ROOT")
            .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        root.join("assets").join(path).is_file()
    } else {
        FILES.iter().any(|(p, _)| *p == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_referenced_asset_is_embedded() {
        assert!(FILES.len() > 30);
        for path in [
            "audio/menu.wav",
            "audio/blaster.ogg",
            "models/blaster.glb",
            "ui/adventure/panel.png",
            "ui/maps/skyreach.png",
        ] {
            assert!(exists(path), "{path} missing");
        }
        for (_, bytes) in FILES {
            assert!(!bytes.is_empty());
        }
    }
}

//! Kenney Platformer Kit models used by the Skyreach map, the flying bots and
//! the first-person blasters.
//!
//! Only the meshes are taken from the glTF files. The kit's models share an
//! external `Textures/colormap.png` palette; when that file is present in
//! `assets/models/Textures/` it is used, otherwise each model gets a flat
//! colour so the map still renders with an incomplete asset pack.
use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState, UntypedAssetId},
    gltf::{Gltf, GltfMesh, GltfNode},
    platform::collections::HashSet,
    prelude::*,
};

const TEXTURE: &str = "models/Textures/colormap.png";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    PlatformLargeGrass,
    Platform,
    WallHigh,
    WallLow,
    Grass,
    GrassSmall,
    Cloud,
    EnemyFlying,
    Blaster,
    BlasterRepeater,
}
impl Model {
    pub const ALL: [Model; 10] = [
        Model::PlatformLargeGrass,
        Model::Platform,
        Model::WallHigh,
        Model::WallLow,
        Model::Grass,
        Model::GrassSmall,
        Model::Cloud,
        Model::EnemyFlying,
        Model::Blaster,
        Model::BlasterRepeater,
    ];
    fn path(self) -> &'static str {
        match self {
            Model::PlatformLargeGrass => "models/platform-large-grass.glb",
            Model::Platform => "models/platform.glb",
            Model::WallHigh => "models/wall-high.glb",
            Model::WallLow => "models/wall-low.glb",
            Model::Grass => "models/grass.glb",
            Model::GrassSmall => "models/grass-small.glb",
            Model::Cloud => "models/cloud.glb",
            Model::EnemyFlying => "models/enemy-flying.glb",
            Model::Blaster => "models/blaster.glb",
            Model::BlasterRepeater => "models/blaster-repeater.glb",
        }
    }
    /// Unscaled bounding size. Every model's origin sits at the bottom centre.
    pub fn size(self) -> Vec3 {
        match self {
            Model::PlatformLargeGrass => Vec3::new(5.2, 0.5, 5.2),
            Model::Platform => Vec3::new(2.2, 0.5, 2.2),
            Model::WallHigh => Vec3::new(1.5, 1.7, 0.7),
            Model::WallLow => Vec3::new(2.2, 0.8, 0.7),
            _ => Vec3::ONE,
        }
    }
    /// Fallback colour used when the kit texture is missing.
    fn flat(self) -> StandardMaterial {
        let (r, g, b) = match self {
            Model::PlatformLargeGrass => (0.36, 0.66, 0.29),
            Model::Platform => (0.62, 0.55, 0.47),
            Model::WallHigh => (0.78, 0.71, 0.6),
            Model::WallLow => (0.7, 0.62, 0.5),
            Model::Grass => (0.42, 0.78, 0.3),
            Model::GrassSmall => (0.5, 0.82, 0.34),
            Model::Cloud => (0.96, 0.98, 1.),
            Model::EnemyFlying => (0.5, 0.52, 0.58),
            Model::Blaster => (0.34, 0.38, 0.45),
            Model::BlasterRepeater => (0.26, 0.3, 0.36),
        };
        StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.85,
            unlit: self == Model::Cloud,
            ..default()
        }
    }
}

#[derive(Resource)]
pub struct Models {
    files: Vec<Handle<Gltf>>,
    texture: Option<Handle<Image>>,
    /// Per model: every mesh primitive with its transform inside the model.
    parts: Vec<Vec<(Handle<Mesh>, Transform)>>,
    materials: Vec<Handle<StandardMaterial>>,
}
impl Models {
    pub fn load(assets: &AssetServer) -> Self {
        let texture = texture_present().then(|| assets.load(TEXTURE));
        Self {
            files: Model::ALL.iter().map(|m| assets.load(m.path())).collect(),
            texture,
            parts: Vec::new(),
            materials: Vec::new(),
        }
    }
    /// Every file the loading screen waits on.
    pub fn handles(&self) -> impl Iterator<Item = UntypedAssetId> + '_ {
        self.files
            .iter()
            .map(|h| h.id().untyped())
            .chain(self.texture.iter().map(|h| h.id().untyped()))
    }
    pub fn material(&self, model: Model) -> Handle<StandardMaterial> {
        self.materials[model as usize].clone()
    }
    pub fn ready(&self) -> bool {
        !self.parts.is_empty()
    }
    /// Spawns the meshes of `model` as children of the entity being built.
    pub fn add(&self, parent: &mut ChildSpawnerCommands, model: Model) {
        self.add_with(parent, model, self.materials[model as usize].clone());
    }
    fn add_with(
        &self,
        parent: &mut ChildSpawnerCommands,
        model: Model,
        material: Handle<StandardMaterial>,
    ) {
        for (mesh, transform) in &self.parts[model as usize] {
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                *transform,
            ));
        }
    }
}

/// Checked up front so a missing optional texture is not requested at all.
fn texture_present() -> bool {
    let root = std::env::var_os("BEVY_ASSET_ROOT")
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    root.join("assets").join(TEXTURE).is_file()
}

/// A file counts as settled once it and its dependencies finished loading, or
/// failed; a missing texture inside a glTF must not hang the loading screen.
pub fn settled(assets: &AssetServer, id: UntypedAssetId) -> bool {
    assets.is_loaded_with_dependencies(id)
        || matches!(assets.get_load_state(id), Some(LoadState::Failed(_)))
        || matches!(
            assets.get_recursive_dependency_load_state(id),
            Some(RecursiveDependencyLoadState::Failed(_))
        )
}

/// Once every file has loaded, flattens each model's node tree into a list of
/// meshes and builds the materials.
pub fn resolve(
    mut models: ResMut<Models>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    meshes: Res<Assets<GltfMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if models.ready() || !models.handles().all(|id| settled(&assets, id)) {
        return;
    }
    let texture = models
        .texture
        .clone()
        .filter(|t| assets.is_loaded_with_dependencies(t.id()));
    let mut parts = Vec::new();
    for (model, file) in Model::ALL.iter().zip(&models.files) {
        let mut list = Vec::new();
        if let Some(gltf) = gltfs.get(file) {
            let children: HashSet<_> = gltf
                .nodes
                .iter()
                .filter_map(|n| nodes.get(n))
                .flat_map(|n| n.children.iter().map(|c| c.id()))
                .collect();
            for root in gltf.nodes.iter().filter(|n| !children.contains(&n.id())) {
                collect(root, Transform::IDENTITY, &nodes, &meshes, &mut list);
            }
        }
        if list.is_empty() {
            warn!("Model {:?} has no meshes or failed to load", model);
        }
        parts.push(list);
    }
    let material = |base: StandardMaterial| match &texture {
        Some(image) => StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(image.clone()),
            ..base
        },
        None => base,
    };
    let built: Vec<_> = Model::ALL
        .iter()
        .map(|m| materials.add(material(m.flat())))
        .collect();
    models.materials = built;
    models.parts = parts;
}

fn collect(
    node: &Handle<GltfNode>,
    parent: Transform,
    nodes: &Assets<GltfNode>,
    meshes: &Assets<GltfMesh>,
    out: &mut Vec<(Handle<Mesh>, Transform)>,
) {
    let Some(node) = nodes.get(node) else {
        return;
    };
    let transform = parent.mul_transform(node.transform);
    if let Some(mesh) = node.mesh.as_ref().and_then(|m| meshes.get(m)) {
        for primitive in &mesh.primitives {
            out.push((primitive.mesh.clone(), transform));
        }
    }
    for child in &node.children {
        collect(child, transform, nodes, meshes, out);
    }
}

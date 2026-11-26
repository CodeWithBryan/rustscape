//! What `LoginManager.loadRegion` does to client state besides moving the
//! scene base: the unchanged-scene test, pending location changes, the scene's
//! spot animations and projectiles, the scripted camera and the map flag.
//!
//! The rebase of players and ground items is `world::rebuild_logical`; the
//! sound queue (`SoundPlayer.size = 0`) belongs to the audio binding and the
//! map reload to the renderer. NPCs need no shift because this client keeps
//! their tiles absolute. `LightingManager.minimapRenderedPlane = -1` (the
//! minimap re-render) is not modelled here.
use crate::{camera, walking::DestinationFlag, world::Scene, world_spot};
use rs09_protocol::game::rebuild::NormalRebuild;

/// `loadRegion`'s first test: a rebuild for the zone the scene is already
/// centred on does nothing at all. The original also compares the plane
/// unless every level is visible (`GlRenderer.enabled ||
/// Preferences.allLevelsVisible`); this client follows the HD renderer, for
/// which that is always true, so the plane does not take part. The scene base
/// is `(zone - 6) * 8`, so equal bases mean equal zones.
pub fn unchanged(base: Option<[i32; 2]>, rebuild: &NormalRebuild) -> bool {
    base == Some(rebuild.base())
}

/// The state `loadRegion` touches after the rebase.
pub struct Parts<'a> {
    /// `ChangeLocRequest.queue`, with absolute tiles.
    pub changes: &'a mut Scene,
    /// `SceneGraph.spotanims` and `SceneGraph.projectiles`.
    pub spots: &'a mut world_spot::State,
    pub camera: &'a mut camera::Script,
    /// `LoginManager.mapFlagX/Y`, scene-local.
    pub flag: &'a mut DestinationFlag,
}

/// Apply a rebuild from `previous` (None for the first scene of a session) to
/// the new scene `base`.
///
/// - Pending location changes outside the new 104 by 104 scene are dropped
///   without being reverted.
/// - The camera returns to the follow type (`Camera.cameraType = 1`); shakes
///   and the stored targets are left as they are.
/// - A visible map flag moves by the base delta so it stays on its tile. One
///   whose X lands on zero becomes hidden, as in the original.
/// - Scene spot animations and projectiles are cleared.
pub fn apply(previous: Option<[i32; 2]>, base: [i32; 2], parts: Parts<'_>) {
    parts.changes.changes.retain(|change| {
        (0..2).all(|axis| (0..104).contains(&(change.slot.tile[axis] - base[axis])))
    });
    parts.camera.locked = false;
    parts.camera.spline = None;
    if let Some(previous) = previous
        && parts.flag.tile[0] != 0
    {
        parts.flag.tile[0] -= base[0] - previous[0];
        parts.flag.tile[1] -= base[1] - previous[1];
    }
    parts.spots.effects.clear();
    parts.spots.projectiles.clear();
}

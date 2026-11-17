//! An instrument for the frame comparison with the original client
//! (`oracle/refcompare`): beside the frame dump, a file named `pose` in the
//! folder `RS09_FRAME_DUMP` names makes the client write `pose.json` there,
//! the camera the current frame is drawn with and the follow-camera state it
//! came from, and remove the request. The comparison pins the original's
//! camera to these values. Nothing happens without the variable and the
//! request file.

/// The parts of the frame's camera that `pose.json` reports.
pub(super) struct Frame<'a> {
    pub pose: &'a rs09_client::camera::Pose,
    pub follow: &'a rs09_client::follow_camera::Follow,
    pub view: &'a rs09_client::projection::Bounds,
    pub view_distance: [i32; 2],
    pub player_local: [i32; 2],
    pub origin: [i32; 2],
    pub plane: usize,
    pub locked: bool,
    /// The spline camera (title screen, CS2 5502), if it is in charge.
    pub spline: Option<&'a rs09_client::camera::Spline>,
}

/// Answers a pending `pose` request, if any.
pub(super) fn answer(frame: Frame<'_>) {
    let Some(folder) = std::env::var_os("RS09_FRAME_DUMP") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    let request = folder.join("pose");
    if !request.exists() {
        return;
    }
    let _ = std::fs::remove_file(&request);
    let follow = frame.follow;
    let view = frame.view;
    let state = serde_json::json!({
        "position": frame.pose.position,
        "pitch": frame.pose.pitch,
        "yaw": frame.pose.yaw,
        "pitch_target": follow.pitch_target,
        "yaw_target": follow.yaw_target,
        "yaw_drift": follow.yaw_drift,
        "orbit_point": follow.position,
        "offset": follow.offset,
        "max_pitch_distance": follow.max_pitch_distance,
        "zoom": follow.zoom,
        "viewport": [view.left, view.top, view.width, view.height],
        "viewport_zoom": view.zoom,
        "view_distance": frame.view_distance,
        "player_fine": frame.player_local,
        "origin": frame.origin,
        "plane": frame.plane,
        "locked": frame.locked,
        "spline": frame.spline.map(|spline| serde_json::json!({
            "position": spline.position,
            "look": spline.look,
            "progress": spline.progress,
            "speed_start": spline.speed_start,
            "speed_end": spline.speed_end,
        })),
    });
    let _ = std::fs::write(folder.join("pose.json"), state.to_string());
}

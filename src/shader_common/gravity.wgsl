// Low-frequency spatial droop. gravity_offset.y is the eased sag depth from the CPU.

fn apply_gravity_droop(position: vec2<f32>) -> vec2<f32> {
    let g = uniforms.gravity;
    if g < 0.001 {
        return position;
    }

    let depth = max(uniforms.gravity_offset.y, 0.0);
    if depth < 0.0001 {
        return position;
    }

    // Hang like a cable: strongest sag near horizontal center.
    let hang = 4.0 * position.x * (1.0 - position.x);
    // Extra weight toward the bottom so it reads as settling downward.
    let bottom_weight = position.y * position.y;
    // Strong spatial mix — depth of ~1.0 moves center features by ~half a screen.
    let sag = depth * (0.85 * hang + 0.65 * bottom_weight + 0.35);

    var warped = position;
    // Sample from above so features appear lower on screen.
    warped.y = warped.y - sag;
    // Gather toward center as mass settles.
    warped.x = warped.x + (0.5 - position.x) * sag * 0.25;
    return warped;
}

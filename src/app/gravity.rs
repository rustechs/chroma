//! Smooth downward *droop* for the shader UV plane.
//!
//! Gravity is a low-frequency spatial sag (hang + settle), not a scrolling
//! camera. Mouse influence can ease the droop up to `MOUSE_FIGHT_CAP` but
//! never cancel it. Fight amount is low-pass filtered so pointer noise does
//! not become high-frequency twitch.

/// Max fraction of droop the mouse may cancel (must stay < 1).
const MOUSE_FIGHT_CAP: f32 = 0.75;
/// Peak UV sag contribution per gravity unit (before soft saturation).
/// gravity≈8 is a strong clear droop; gravity≈50 is extreme.
const BASE_DROOP: f32 = 0.12;
/// Soft ceiling on eased droop depth so huge gravity stays stable.
const MAX_DROOP: f32 = 1.35;
/// Seconds to approach the target droop (higher = heavier, slower fall).
const DROOP_TAU_SECONDS: f32 = 0.35;
/// Seconds to smooth mouse fight (kills twitch from influence flicker).
const FIGHT_TAU_SECONDS: f32 = 0.35;
/// `mouse_influence` above hover level counts as pressed (see `mouse.rs`).
const MOUSE_PRESS_INFLUENCE_THRESHOLD: f32 = 1.01;
const HOVER_FIGHT_FACTOR: f32 = 0.65;

#[derive(Debug, Clone)]
pub struct GravityState {
  /// `[0, droop_depth]` — only Y is used by the shader droop warp.
  pub offset: [f32; 2],
  /// Unused; kept so call sites/tests that touch velocity still compile cleanly.
  pub velocity: [f32; 2],
  smoothed_fight: f32,
}

impl Default for GravityState {
  fn default() -> Self {
    Self {
      offset: [0.0, 0.0],
      velocity: [0.0, 0.0],
      smoothed_fight: 0.0,
    }
  }
}

impl GravityState {
  /// Ease droop depth toward a mouse-fought target. No oscillatory integration.
  pub fn update(
    &mut self,
    gravity: f32,
    mouse_fight: f32,
    _mouse_x: f32,
    _mouse_y: f32,
    mouse_influence: f32,
    delta_time: f32,
  ) {
    let dt = delta_time.clamp(0.0, 0.1);
    let gravity = gravity.max(0.0);

    let raw_fight = raw_mouse_fight(mouse_fight, mouse_influence);
    let fight_alpha = 1.0 - (-dt / FIGHT_TAU_SECONDS).exp();
    self.smoothed_fight += (raw_fight - self.smoothed_fight) * fight_alpha;

    let target_droop = if gravity <= 0.0001 {
      0.0
    } else {
      (gravity * BASE_DROOP * (1.0 - self.smoothed_fight)).min(MAX_DROOP)
    };

    let droop_alpha = 1.0 - (-dt / DROOP_TAU_SECONDS).exp();
    let droop = self.offset[1] + (target_droop - self.offset[1]) * droop_alpha;
    self.offset[1] = droop;
    self.offset[0] = 0.0;
    // Expose approach rate as a soft "velocity" for debugging/tests.
    self.velocity[1] = (target_droop - droop) / DROOP_TAU_SECONDS;
    self.velocity[0] = 0.0;
  }
}

fn raw_mouse_fight(mouse_fight: f32, mouse_influence: f32) -> f32 {
  if mouse_influence <= 0.001 {
    return 0.0;
  }

  let press_boost = if mouse_influence > MOUSE_PRESS_INFLUENCE_THRESHOLD {
    1.0
  } else {
    HOVER_FIGHT_FACTOR
  };
  let strength = mouse_fight.clamp(0.0, 1.0) * press_boost;
  // Map influence 0..~1.75 into 0..1 before applying the hard cap.
  let influence_t = (mouse_influence / 1.75).clamp(0.0, 1.0);
  (strength * influence_t * MOUSE_FIGHT_CAP).clamp(0.0, MOUSE_FIGHT_CAP)
}

/// Target droop depth after mouse fight (no smoothing) — for tests.
#[cfg(test)]
fn target_droop(gravity: f32, mouse_fight: f32, mouse_influence: f32) -> f32 {
  let gravity = gravity.max(0.0);
  if gravity <= 0.0001 {
    return 0.0;
  }
  (gravity * BASE_DROOP * (1.0 - raw_mouse_fight(mouse_fight, mouse_influence))).min(MAX_DROOP)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mouse_cannot_overpower_gravity() {
    let without = target_droop(1.0, 1.0, 0.0);
    let with_mouse = target_droop(1.0, 1.0, 1.75);

    assert!(without > 0.0);
    assert!(with_mouse > 0.0);
    assert!(with_mouse < without);
    assert!(
      with_mouse + f32::EPSILON >= without * (1.0 - MOUSE_FIGHT_CAP),
      "mouse fight exceeded cap: {with_mouse} vs floor {}",
      without * (1.0 - MOUSE_FIGHT_CAP)
    );
  }

  #[test]
  fn inactive_mouse_leaves_full_droop() {
    let droop = target_droop(0.8, 1.0, 0.0);
    assert!((droop - 0.8 * BASE_DROOP).abs() < 1e-5);
  }

  #[test]
  fn update_eases_downward_droop_over_time() {
    let mut state = GravityState::default();
    for _ in 0..90 {
      state.update(1.0, 0.7, 0.5, 0.5, 0.0, 1.0 / 60.0);
    }
    assert!(
      state.offset[1] > BASE_DROOP * 0.5,
      "expected settled droop, got {}",
      state.offset[1]
    );
  }

  #[test]
  fn gravity_zero_eases_droop_away() {
    let mut state = GravityState {
      offset: [0.0, 0.2],
      ..Default::default()
    };

    for _ in 0..180 {
      state.update(0.0, 1.0, 0.2, 0.2, 1.75, 1.0 / 60.0);
    }

    assert!(state.offset[1].abs() < 0.02, "droop={}", state.offset[1]);
  }

  #[test]
  fn pressed_mouse_reduces_but_does_not_clear_droop() {
    let falling = target_droop(1.0, 1.0, 0.0);
    let fighting = target_droop(1.0, 1.0, 1.75);

    assert!(fighting > 0.0);
    assert!(fighting < falling);
  }

  #[test]
  fn mouse_influence_scales_fight() {
    let hover = target_droop(1.0, 1.0, 1.0);
    let pressed = target_droop(1.0, 1.0, 1.75);

    assert!(pressed < hover);
    assert!(pressed > 0.0);
  }

  #[test]
  fn fight_smoothing_avoids_instant_jumps() {
    let mut state = GravityState::default();
    state.update(1.0, 1.0, 0.5, 0.5, 0.0, 1.0 / 60.0);
    let baseline = state.offset[1];

    // One frame of full press should not instantly collapse droop.
    state.update(1.0, 1.0, 0.5, 0.2, 1.75, 1.0 / 60.0);
    assert!(
      (state.offset[1] - baseline).abs() < BASE_DROOP * 0.15,
      "droop jumped too hard in one frame: {} -> {}",
      baseline,
      state.offset[1]
    );
  }
}

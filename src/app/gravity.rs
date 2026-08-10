//! Downward gravity for the shader UV plane. Mouse fight strength is read from
//! `ShaderParams` (`mouse_x`/`mouse_y`/`mouse_influence`) — no parallel tracker.

/// Max fraction of gravity acceleration the mouse may cancel (must stay < 1).
const MOUSE_FIGHT_CAP: f32 = 0.75;
/// Fight strength when the cursor is active but not held.
const HOVER_FIGHT_FACTOR: f32 = 0.65;
/// `mouse_influence` above hover level counts as pressed (see `mouse.rs`).
const MOUSE_PRESS_INFLUENCE_THRESHOLD: f32 = 1.01;
/// UV units / s² per unit of the `gravity` parameter.
const GRAVITY_ACCEL_SCALE: f32 = 0.45;
/// Horizontal pull toward the mouse cursor (scaled by gravity).
const HORIZONTAL_PULL: f32 = 0.55;
/// Velocity damping per second (exponential decay coefficient).
const DAMPING: f32 = 2.4;
/// Soft speed clamp in UV units / s.
const MAX_SPEED: f32 = 1.25;

#[derive(Debug, Clone)]
pub struct GravityState {
  pub offset: [f32; 2],
  pub velocity: [f32; 2],
}

impl Default for GravityState {
  fn default() -> Self {
    Self {
      offset: [0.0, 0.0],
      velocity: [0.0, 0.0],
    }
  }
}

impl GravityState {
  /// Integrate gravity and mouse forces for one frame.
  ///
  /// Mouse vertical lift is hard-capped so net downward acceleration never
  /// drops below `gravity_accel * (1 - MOUSE_FIGHT_CAP)`.
  pub fn update(
    &mut self,
    gravity: f32,
    mouse_fight: f32,
    mouse_x: f32,
    _mouse_y: f32,
    mouse_influence: f32,
    delta_time: f32,
  ) {
    let dt = delta_time.clamp(0.0, 0.1);
    let mouse_active = mouse_influence > 0.001;
    let mouse_pressed = mouse_influence > MOUSE_PRESS_INFLUENCE_THRESHOLD;

    let gravity = gravity.max(0.0);
    if gravity <= 0.0001 {
      // Ease residual motion back to rest when gravity is off.
      self.velocity[0] *= (-DAMPING * dt).exp();
      self.velocity[1] *= (-DAMPING * dt).exp();
      self.offset[0] += self.velocity[0] * dt;
      self.offset[1] += self.velocity[1] * dt;
      return;
    }

    let mut accel_x = 0.0;
    let accel_y = net_vertical_accel(gravity, mouse_fight, mouse_active, mouse_pressed);

    if mouse_active {
      let press_boost = if mouse_pressed {
        1.0
      } else {
        HOVER_FIGHT_FACTOR
      };
      // Horizontal tug toward the cursor; strength scales with gravity.
      accel_x += (mouse_x - 0.5) * HORIZONTAL_PULL * gravity * press_boost;
    }

    self.velocity[0] += accel_x * dt;
    self.velocity[1] += accel_y * dt;

    let damp = (-DAMPING * dt).exp();
    self.velocity[0] *= damp;
    self.velocity[1] *= damp;

    let speed = (self.velocity[0] * self.velocity[0] + self.velocity[1] * self.velocity[1]).sqrt();
    if speed > MAX_SPEED {
      let scale = MAX_SPEED / speed;
      self.velocity[0] *= scale;
      self.velocity[1] *= scale;
    }

    self.offset[0] += self.velocity[0] * dt;
    self.offset[1] += self.velocity[1] * dt;
  }
}

/// Net vertical acceleration after mouse fight.
fn net_vertical_accel(
  gravity: f32,
  mouse_fight: f32,
  mouse_active: bool,
  mouse_pressed: bool,
) -> f32 {
  let gravity = gravity.max(0.0);
  let gravity_accel = gravity * GRAVITY_ACCEL_SCALE;
  if gravity_accel <= 0.0 {
    return 0.0;
  }
  if !mouse_active {
    return gravity_accel;
  }

  let fight_strength = mouse_fight.clamp(0.0, 1.0);
  let press_boost = if mouse_pressed {
    1.0
  } else {
    HOVER_FIGHT_FACTOR
  };
  let requested_fight = gravity_accel * MOUSE_FIGHT_CAP * fight_strength * press_boost;
  let min_net_accel = gravity_accel * (1.0 - MOUSE_FIGHT_CAP);
  (gravity_accel - requested_fight).max(min_net_accel)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mouse_cannot_overpower_gravity() {
    let gravity = 1.0;
    let with_mouse = net_vertical_accel(gravity, 1.0, true, true);
    let without = net_vertical_accel(gravity, 1.0, false, false);

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
  fn inactive_mouse_leaves_full_gravity() {
    let accel = net_vertical_accel(0.8, 1.0, false, false);
    assert!((accel - 0.8 * GRAVITY_ACCEL_SCALE).abs() < 1e-5);
  }

  #[test]
  fn update_increases_downward_offset_over_time() {
    let mut state = GravityState::default();
    for _ in 0..30 {
      state.update(1.0, 0.7, 0.5, 0.5, 0.0, 1.0 / 60.0);
    }
    assert!(state.offset[1] > 0.0);
    assert!(state.velocity[1] > 0.0);
  }

  #[test]
  fn gravity_zero_eases_residual_motion() {
    let mut state = GravityState {
      velocity: [0.1, 0.2],
      ..Default::default()
    };

    for _ in 0..90 {
      state.update(0.0, 1.0, 0.2, 0.2, 1.75, 1.0 / 60.0);
    }

    assert!(state.offset[1].abs() < 0.2);
    assert!(state.velocity[1].abs() < 0.05);
  }

  #[test]
  fn pressed_mouse_slows_but_does_not_reverse_fall() {
    let mut falling = GravityState::default();
    let mut fighting = GravityState::default();

    for _ in 0..45 {
      falling.update(1.0, 1.0, 0.5, 0.5, 0.0, 1.0 / 60.0);
      fighting.update(1.0, 1.0, 0.5, 0.2, 1.75, 1.0 / 60.0);
    }

    assert!(fighting.offset[1] > 0.0);
    assert!(fighting.offset[1] < falling.offset[1]);
    assert!(fighting.velocity[1] > 0.0);
  }

  #[test]
  fn mouse_influence_drives_fight_from_shader_params() {
    let hover = net_vertical_accel(1.0, 1.0, true, false);
    let pressed = net_vertical_accel(1.0, 1.0, true, true);

    assert!(pressed < hover);
    assert!(pressed > 0.0);
  }
}

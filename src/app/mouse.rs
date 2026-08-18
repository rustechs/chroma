use anyhow::Result;
use chroma::{
  ascii::{AsciiConverter, AsciiPalette},
  params::{MouseInertia, ShaderParams},
};
use crossterm::{
  event::{MouseButton, MouseEvent, MouseEventKind},
  terminal,
};

use super::{input, DebugLog};

const MOUSE_HOVER_INFLUENCE: f32 = 1.0;
const MOUSE_PRESS_INFLUENCE: f32 = 1.75;
const MOUSE_HUE_DRAG_SCALE: f32 = 180.0;
pub(crate) const MOUSE_SCROLL_SCALE_STEP: f32 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
  Idle,
  Returning,
  Entering,
  Tracking,
}

#[derive(Debug, Clone)]
pub struct MouseMotionState {
  pub mode: MouseMode,
  pub inertia: MouseInertia,
  pub target_x: f32,
  pub target_y: f32,
  pub target_influence: f32,
  /// After FocusGained, wait for the first mouse event before coasting in.
  pub pending_enter: bool,
}

impl Default for MouseMotionState {
  fn default() -> Self {
    Self {
      mode: MouseMode::Idle,
      inertia: MouseInertia::default(),
      target_x: 0.5,
      target_y: 0.5,
      target_influence: 0.0,
      pending_enter: false,
    }
  }
}

impl MouseMotionState {
  pub fn begin_return(&mut self, params: &mut ShaderParams) {
    if !params.should_begin_mouse_return() {
      params.clear_mouse_interaction();
      *self = Self::default();
      return;
    }

    self.mode = MouseMode::Returning;
    self.pending_enter = false;
    self.target_x = 0.5;
    self.target_y = 0.5;
    self.target_influence = 0.0;
  }

  pub fn mark_focus_gained(&mut self) {
    self.pending_enter = true;
  }

  pub fn tick(&mut self, params: &mut ShaderParams, delta_time: f32) {
    match self.mode {
      MouseMode::Idle => {}
      MouseMode::Returning => {
        if !self.integrate(params, delta_time) {
          params.clear_mouse_interaction();
          *self = Self::default();
        }
      }
      MouseMode::Entering => {
        if !self.integrate(params, delta_time) {
          self.mode = MouseMode::Tracking;
        }
      }
      MouseMode::Tracking => {
        self.integrate(params, delta_time);
      }
    }
  }

  fn integrate(&mut self, params: &mut ShaderParams, delta_time: f32) -> bool {
    params.tick_mouse_inertia(
      delta_time,
      self.target_x,
      self.target_y,
      self.target_influence,
      &mut self.inertia,
    )
  }

  fn begin_enter(&mut self, target_x: f32, target_y: f32, target_influence: f32) {
    self.mode = MouseMode::Entering;
    self.pending_enter = false;
    self.target_x = target_x;
    self.target_y = target_y;
    self.target_influence = target_influence;
  }
}

pub(crate) fn handle_mouse_event(
  mouse_event: MouseEvent,
  params: &mut ShaderParams,
  converter: &mut AsciiConverter,
  show_status_bar: bool,
  debug_log: &mut DebugLog,
  motion: &mut MouseMotionState,
) -> Result<()> {
  let (term_width, term_height) = terminal::size()?;
  let (target_x, target_y) = ShaderParams::mouse_uv_from_terminal(
    mouse_event.column,
    mouse_event.row,
    term_width,
    term_height,
    show_status_bar,
  );

  let previous_x = motion.target_x;
  let previous_y = motion.target_y;

  let wants_inertial_enter = motion.pending_enter
    || matches!(
      motion.mode,
      MouseMode::Idle | MouseMode::Returning | MouseMode::Entering
    );

  let target_influence = match mouse_event.kind {
    MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
      MOUSE_PRESS_INFLUENCE
    }
    MouseEventKind::Up(MouseButton::Left) | MouseEventKind::Up(MouseButton::Right) => {
      MOUSE_HOVER_INFLUENCE
    }
    MouseEventKind::Drag(MouseButton::Left) => MOUSE_PRESS_INFLUENCE,
    _ => MOUSE_HOVER_INFLUENCE,
  };

  match mouse_event.kind {
    MouseEventKind::Down(MouseButton::Left) => {
      params.randomize();
      converter.set_palette(AsciiPalette::from(params.palette));
      debug_logln!(debug_log, "MOUSE: Left click randomized parameters")?;
    }
    MouseEventKind::Down(MouseButton::Right) => {
      input::cycle_effect(params, debug_log)?;
    }
    MouseEventKind::Drag(MouseButton::Left) => {
      let dx = target_x - previous_x;
      let dy = target_y - previous_y;
      params.adjust_hue(dx * MOUSE_HUE_DRAG_SCALE);
      params.adjust_scale(-dy * 2.0);
    }
    MouseEventKind::ScrollUp => {
      params.adjust_scale(MOUSE_SCROLL_SCALE_STEP);
    }
    MouseEventKind::ScrollDown => {
      params.adjust_scale(-MOUSE_SCROLL_SCALE_STEP);
    }
    _ => {}
  }

  if wants_inertial_enter {
    if motion.mode != MouseMode::Entering {
      motion.begin_enter(target_x, target_y, target_influence);
    } else {
      motion.target_x = target_x;
      motion.target_y = target_y;
      motion.target_influence = target_influence;
      motion.pending_enter = false;
    }
  } else {
    motion.mode = MouseMode::Tracking;
    motion.pending_enter = false;
    motion.target_x = target_x;
    motion.target_y = target_y;
    motion.target_influence = target_influence;
  }

  params.clamp_all();

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_focus_gained_then_mouse_starts_enter_mode() {
    let mut motion = MouseMotionState::default();
    motion.mark_focus_gained();
    assert!(motion.pending_enter);

    motion.begin_enter(0.75, 0.25, MOUSE_HOVER_INFLUENCE);
    assert_eq!(motion.mode, MouseMode::Entering);
    assert!(!motion.pending_enter);
    assert!((motion.target_x - 0.75).abs() < f32::EPSILON);
  }

  #[test]
  fn test_scroll_scale_step_constant() {
    let mut params = ShaderParams {
      scale: 1.0,
      ..ShaderParams::default()
    };
    params.adjust_scale(MOUSE_SCROLL_SCALE_STEP);
    assert!((params.scale - 1.15).abs() < 1e-5);
  }

  #[test]
  fn test_tracking_coasts_toward_cursor_without_teleporting() {
    let mut params = ShaderParams::default();
    let mut motion = MouseMotionState {
      mode: MouseMode::Tracking,
      target_x: 0.9,
      target_y: 0.1,
      target_influence: MOUSE_HOVER_INFLUENCE,
      ..MouseMotionState::default()
    };

    motion.tick(&mut params, 1.0 / 60.0);

    assert_eq!(motion.mode, MouseMode::Tracking);
    assert!(params.mouse_x > 0.5);
    assert!(params.mouse_x < 0.58);
    assert!(motion.inertia.vel_x > 0.0);
  }

  #[test]
  fn test_return_preserves_existing_velocity() {
    let mut params = ShaderParams {
      mouse_x: 0.8,
      mouse_y: 0.4,
      mouse_influence: 1.0,
      ..ShaderParams::default()
    };
    let mut motion = MouseMotionState {
      mode: MouseMode::Tracking,
      inertia: MouseInertia {
        vel_x: 1.75,
        vel_y: -0.4,
        vel_influence: 0.2,
      },
      target_x: 0.8,
      target_y: 0.4,
      target_influence: 1.0,
      ..MouseMotionState::default()
    };

    motion.begin_return(&mut params);

    assert_eq!(motion.mode, MouseMode::Returning);
    assert!((motion.inertia.vel_x - 1.75).abs() < f32::EPSILON);
    assert!((motion.inertia.vel_y + 0.4).abs() < f32::EPSILON);
    assert!((motion.inertia.vel_influence - 0.2).abs() < f32::EPSILON);
  }
}

use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{randomizer, ColorMode, PaletteType, PatternType};

fn default_mouse_center() -> f32 {
  0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderParams {
  pub time: f32,
  pub resolution_width: u32,
  pub resolution_height: u32,

  pub frequency: f32,
  pub amplitude: f32,
  pub speed: f32,
  pub color_shift: f32,
  pub scale: f32,
  pub octaves: u32,

  pub noise_strength: f32,
  pub distort_amplitude: f32,
  pub noise_scale: f32,
  pub z_rate: f32,

  pub brightness: f32,
  pub contrast: f32,
  pub hue: f32,
  pub saturation: f32,
  pub gamma: f32,

  pub vignette: f32,
  pub vignette_softness: f32,
  pub glyph_sharpness: f32,

  pub background_tint_r: f32,
  pub background_tint_g: f32,
  pub background_tint_b: f32,

  pub terminal_bg_r: f32,
  pub terminal_bg_g: f32,
  pub terminal_bg_b: f32,

  pub palette: PaletteType,
  pub color_mode: ColorMode,
  pub pattern_type: PatternType,

  pub audio_enabled: bool,
  pub bass_influence: f32,
  pub mid_influence: f32,
  pub treble_influence: f32,
  pub beat_sensitivity: f32,

  pub effect_time: f32,
  pub effect_type: u32,

  pub beat_distortion_time: f32,
  pub beat_distortion_strength: f32,
  pub beat_zoom_strength: f32,

  /// Normalized mouse X in shader UV space (0–1). Runtime-only; not saved to configs.
  #[serde(skip_serializing, default = "default_mouse_center")]
  pub mouse_x: f32,
  /// Normalized mouse Y in shader UV space (0–1). Runtime-only; not saved to configs.
  #[serde(skip_serializing, default = "default_mouse_center")]
  pub mouse_y: f32,
  /// How strongly visuals react to the cursor (0 = off, 1 = hover, >1 = pressed).
  #[serde(skip_serializing, default)]
  pub mouse_influence: f32,
}

impl Default for ShaderParams {
  fn default() -> Self {
    Self {
      time: 0.0,
      resolution_width: 80,
      resolution_height: 24,

      frequency: 10.0,
      amplitude: 1.0,
      speed: 0.5,
      color_shift: 0.0,
      scale: 1.0,
      octaves: 4,

      noise_strength: 0.15,
      distort_amplitude: 0.5,
      noise_scale: 0.005,
      z_rate: 0.02,

      brightness: 1.2,
      contrast: 1.0,
      hue: 0.0,
      saturation: 1.0,
      gamma: 1.0,

      vignette: 0.3,
      vignette_softness: 0.5,
      glyph_sharpness: 1.0,

      background_tint_r: 0.0,
      background_tint_g: 0.0,
      background_tint_b: 0.0,

      terminal_bg_r: 0.0,
      terminal_bg_g: 0.0,
      terminal_bg_b: 0.0,

      palette: PaletteType::Simple,
      color_mode: ColorMode::Chromatic,
      pattern_type: PatternType::Plasma,

      audio_enabled: true,
      bass_influence: 0.5,
      mid_influence: 0.3,
      treble_influence: 0.2,
      beat_sensitivity: 1.0, // Default balanced sensitivity

      effect_time: -100.0,
      effect_type: 0,

      beat_distortion_time: -100.0,
      beat_distortion_strength: 0.85,
      beat_zoom_strength: 0.7,

      mouse_x: 0.5,
      mouse_y: 0.5,
      mouse_influence: 0.0,
    }
  }
}

impl ShaderParams {
  fn adjust_clamped(current: &mut f32, delta: f32, min: f32, max: f32) {
    *current = (*current + delta).clamp(min, max);
  }

  fn normalized_hue(hue: f32) -> f32 {
    let normalized = hue % 360.0;

    if normalized < 0.0 {
      normalized + 360.0
    } else {
      normalized
    }
  }

  fn config_hash(&self) -> String {
    let toml_string = toml::to_string(self).unwrap_or_default();
    let mut hasher = Sha256::new();

    hasher.update(toml_string.as_bytes());

    let result = hasher.finalize();
    let bytes: &[u8] = result.as_ref();

    bytes
      .iter()
      .take(6)
      .map(|byte| format!("{byte:02x}"))
      .collect()
  }

  fn config_filename(&self) -> String {
    format!("config_{}.toml", self.config_hash())
  }

  fn config_path_in<P: AsRef<Path>>(&self, directory: P) -> PathBuf {
    directory.as_ref().join(self.config_filename())
  }

  /// Configure for audio-reactive mode with calm initial state
  /// Starts nearly still and dimmed, waiting for audio to bring it to life
  pub fn with_audio_reactive_defaults() -> Self {
    Self {
      speed: 0.05,                   // Nearly still (vs default 1.0)
      brightness: 0.6,               // Dimmed (vs default 1.2)
      contrast: 0.8,                 // Softer (vs default 1.0)
      amplitude: 0.4,                // Minimal (vs default 1.0)
      frequency: 6.0,                // Lower detail (vs default 10.0)
      audio_enabled: true,           // Audio reactive mode ON
      effect_time: -100.0,           // Far in past to prevent startup wave
      beat_distortion_time: -100.0,  // Far in past to prevent startup distortion
      beat_distortion_strength: 0.8, // Default beat pop strength
      beat_zoom_strength: 0.0,       // Zoom strength (set per-beat)
      ..Default::default()
    }
  }

  pub fn update_time(&mut self, delta_time: f32) {
    self.time += delta_time * self.speed;
  }

  pub fn set_resolution(&mut self, width: u32, height: u32) {
    self.resolution_width = width;
    self.resolution_height = height;
  }

  pub fn apply_audio_data(&mut self, bass: f32, mid: f32, treble: f32) {
    self.audio_enabled = true;
    self.amplitude = 1.0 + bass * self.bass_influence;
    self.color_shift = mid * self.mid_influence;
    self.frequency = 1.0 + treble * self.treble_influence;
  }

  pub fn clamp_all(&mut self) {
    self.audio_enabled = true;

    self.frequency = self.frequency.clamp(3.0, 18.0);
    self.amplitude = self.amplitude.clamp(0.0, 2.0);
    self.speed = self.speed.clamp(0.0, 1.0);
    self.scale = self.scale.clamp(0.1, 5.0);

    self.noise_strength = self.noise_strength.clamp(0.0, 0.5);
    self.distort_amplitude = self.distort_amplitude.clamp(0.0, 2.0);
    self.noise_scale = self.noise_scale.clamp(0.0, 0.01);
    self.z_rate = self.z_rate.clamp(0.0, 0.1);

    self.brightness = self.brightness.clamp(0.0, 2.0);
    self.contrast = self.contrast.clamp(0.2, 2.0);
    self.hue = Self::normalized_hue(self.hue);

    self.saturation = self.saturation.clamp(0.0, 2.0);
    self.gamma = self.gamma.clamp(0.5, 2.0);

    self.vignette = self.vignette.clamp(0.0, 1.0);
    self.vignette_softness = self.vignette_softness.clamp(0.0, 1.0);
    self.glyph_sharpness = self.glyph_sharpness.clamp(0.5, 2.0);

    self.background_tint_r = self.background_tint_r.clamp(0.0, 1.0);
    self.background_tint_g = self.background_tint_g.clamp(0.0, 1.0);
    self.background_tint_b = self.background_tint_b.clamp(0.0, 1.0);

    self.terminal_bg_r = self.terminal_bg_r.clamp(0.0, 1.0);
    self.terminal_bg_g = self.terminal_bg_g.clamp(0.0, 1.0);
    self.terminal_bg_b = self.terminal_bg_b.clamp(0.0, 1.0);

    self.bass_influence = self.bass_influence.clamp(0.0, 1.0);
    self.mid_influence = self.mid_influence.clamp(0.0, 1.0);
    self.treble_influence = self.treble_influence.clamp(0.0, 1.0);
    self.beat_sensitivity = self.beat_sensitivity.clamp(0.1, 3.0);

    self.mouse_x = self.mouse_x.clamp(0.0, 1.0);
    self.mouse_y = self.mouse_y.clamp(0.0, 1.0);
    self.mouse_influence = self.mouse_influence.clamp(0.0, 2.0);
  }

  /// Update mouse UV from terminal cell coordinates.
  pub fn set_mouse_from_terminal(
    &mut self,
    column: u16,
    row: u16,
    terminal_width: u16,
    terminal_height: u16,
    show_status_bar: bool,
  ) {
    let (mouse_x, mouse_y) = Self::mouse_uv_from_terminal(
      column,
      row,
      terminal_width,
      terminal_height,
      show_status_bar,
    );
    self.mouse_x = mouse_x;
    self.mouse_y = mouse_y;
  }

  /// Restore screen-center defaults so shaders behave as if the mouse is inactive.
  pub fn clear_mouse_interaction(&mut self) {
    self.mouse_x = 0.5;
    self.mouse_y = 0.5;
    self.mouse_influence = 0.0;
  }

  /// Soft spring step toward a mouse UV / influence target. Returns true while still animating.
  pub fn tick_mouse_spring(
    &mut self,
    delta_time: f32,
    target_x: f32,
    target_y: f32,
    target_influence: f32,
    vel_x: &mut f32,
    vel_y: &mut f32,
  ) -> bool {
    let dt = delta_time.clamp(0.0, 0.05);

    // Faster underdamped spring: soft bounce, settles in ~0.25–0.35s.
    const STIFFNESS: f32 = 48.0;
    const DAMPING: f32 = 12.5;
    const INFLUENCE_RATE: f32 = 10.0;

    let accel_x = STIFFNESS * (target_x - self.mouse_x) - DAMPING * *vel_x;
    let accel_y = STIFFNESS * (target_y - self.mouse_y) - DAMPING * *vel_y;
    *vel_x += accel_x * dt;
    *vel_y += accel_y * dt;
    self.mouse_x += *vel_x * dt;
    self.mouse_y += *vel_y * dt;

    let influence_t = 1.0 - (-INFLUENCE_RATE * dt).exp();
    self.mouse_influence += (target_influence - self.mouse_influence) * influence_t;

    let settled = (self.mouse_x - target_x).abs() < 0.003
      && (self.mouse_y - target_y).abs() < 0.003
      && (self.mouse_influence - target_influence).abs() < 0.02
      && vel_x.abs() < 0.04
      && vel_y.abs() < 0.04;

    if settled {
      self.mouse_x = target_x.clamp(0.0, 1.0);
      self.mouse_y = target_y.clamp(0.0, 1.0);
      self.mouse_influence = target_influence.clamp(0.0, 2.0);
      *vel_x = 0.0;
      *vel_y = 0.0;
      false
    } else {
      self.mouse_x = self.mouse_x.clamp(0.0, 1.0);
      self.mouse_y = self.mouse_y.clamp(0.0, 1.0);
      self.mouse_influence = self.mouse_influence.clamp(0.0, 2.0);
      true
    }
  }

  pub fn should_begin_mouse_return(&self) -> bool {
    self.mouse_influence > 0.01
      || (self.mouse_x - 0.5).abs() >= 0.001
      || (self.mouse_y - 0.5).abs() >= 0.001
  }

  /// Map terminal cell coordinates to normalized mouse UV without mutating state.
  pub fn mouse_uv_from_terminal(
    column: u16,
    row: u16,
    terminal_width: u16,
    terminal_height: u16,
    show_status_bar: bool,
  ) -> (f32, f32) {
    let shader_height = if show_status_bar {
      terminal_height.saturating_sub(1).max(1)
    } else {
      terminal_height.max(1)
    };
    let shader_width = terminal_width.max(1);

    let mouse_x = (column as f32 / shader_width as f32).clamp(0.0, 1.0);
    let mouse_y = (row as f32 / shader_height as f32).min(1.0).clamp(0.0, 1.0);
    (mouse_x, mouse_y)
  }

  pub fn adjust_frequency(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.frequency, delta, 3.0, 18.0);
  }

  pub fn adjust_amplitude(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.amplitude, delta, 0.0, 2.0);
  }

  pub fn adjust_speed(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.speed, delta, 0.0, 1.0);
  }

  pub fn adjust_scale(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.scale, delta, 0.1, 5.0);
  }

  pub fn adjust_brightness(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.brightness, delta, 0.0, 2.0);
  }

  pub fn adjust_contrast(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.contrast, delta, 0.2, 2.0);
  }

  pub fn adjust_saturation(&mut self, delta: f32) {
    Self::adjust_clamped(&mut self.saturation, delta, 0.0, 2.0);
  }

  pub fn adjust_hue(&mut self, delta: f32) {
    self.hue = Self::normalized_hue(self.hue + delta);
  }

  pub fn randomize(&mut self) {
    randomizer::randomize(self);
  }

  pub fn randomize_with_seed(&mut self, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);

    randomizer::randomize_with_rng(self, &mut rng);
  }

  pub fn save_to_file(&self) -> Result<String> {
    let filename = self.config_filename();

    self.save_to_file_in(".")?;

    Ok(filename)
  }

  pub fn save_to_file_in<P: AsRef<Path>>(&self, directory: P) -> Result<PathBuf> {
    let path = self.config_path_in(&directory);

    if path.exists() {
      return Ok(path);
    }

    let toml_content =
      toml::to_string_pretty(self).context("Failed to serialize configuration to TOML")?;

    fs::create_dir_all(directory.as_ref()).context(format!(
      "Failed to create config directory: {}",
      directory.as_ref().display()
    ))?;
    fs::write(&path, toml_content)
      .context(format!("Failed to write config file: {}", path.display()))?;

    Ok(path)
  }

  pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    let content = fs::read_to_string(path.as_ref()).context(format!(
      "Failed to read config file: {}",
      path.as_ref().display()
    ))?;

    Self::load_from_str(&content)
  }

  /// Load shader params from a TOML string, merging with defaults for missing fields.
  pub fn load_from_str(content: &str) -> Result<Self> {
    // Start with defaults, then deserialize on top (missing fields keep defaults)
    let default_params = Self::default();
    let default_toml = toml::to_string(&default_params)?;
    let mut default_value: toml::Value = toml::from_str(&default_toml)?;

    // Parse the loaded config
    let loaded_value: toml::Value =
      toml::from_str(content).context("Failed to parse config as TOML")?;

    // Merge loaded config into defaults (only overwrites present fields)
    if let (toml::Value::Table(ref mut default_table), toml::Value::Table(loaded_table)) =
      (&mut default_value, loaded_value)
    {
      for (key, value) in loaded_table {
        default_table.insert(key, value);
      }
    }

    // Deserialize merged config
    let mut params: ShaderParams = toml::from_str(&toml::to_string(&default_value)?)?;

    params.clamp_all();

    Ok(params)
  }
}

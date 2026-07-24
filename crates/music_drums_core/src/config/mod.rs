//! Shareable drums detection / mapping presets (JSON).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectorMode {
    /// Original multi-band envelope flux (pre-house tuning).
    #[default]
    Multiband,
    /// Low-pass + bass-dominance gate (house / electronic).
    Kick,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorSettings {
    pub mode: DetectorMode,
    /// Refractory period after a hit (ms).
    pub cooldown_ms: f32,
    /// Flux history window length (samples of flux, not audio).
    pub history_len: usize,
    /// Multiband: absolute flux floor before / sensitivity.
    pub flux_base: f32,
    /// Multiband: mean flux multiplier.
    pub flux_mean_mul: f32,
    pub low_weight: f32,
    pub mid_weight: f32,
    pub high_weight: f32,
    /// Kick mode: low-pass cutoff Hz.
    pub lp_hz: f32,
    /// Kick mode: min bass/broadband ratio.
    pub bass_ratio_min: f32,
    /// Kick mode: threshold = flux_base + mean * (floor_mul_base + (1-sens)*span)
    pub floor_mul_base: f32,
    pub floor_mul_sens_span: f32,
    /// Kick mode: shorten cooldown as sensitivity rises.
    pub cooldown_sens_span_ms: f32,
}

impl Default for DetectorSettings {
    fn default() -> Self {
        classic_detector()
    }
}

fn classic_detector() -> DetectorSettings {
    DetectorSettings {
        mode: DetectorMode::Multiband,
        cooldown_ms: 80.0,
        history_len: 48,
        flux_base: 0.002,
        flux_mean_mul: 2.5,
        low_weight: 1.6,
        mid_weight: 1.1,
        high_weight: 0.7,
        lp_hz: 140.0,
        bass_ratio_min: 0.55,
        floor_mul_base: 3.2,
        floor_mul_sens_span: 3.5,
        cooldown_sens_span_ms: 0.0,
    }
}

fn house_detector() -> DetectorSettings {
    DetectorSettings {
        mode: DetectorMode::Kick,
        cooldown_ms: 100.0,
        history_len: 96,
        flux_base: 0.0008,
        flux_mean_mul: 3.2,
        low_weight: 1.6,
        mid_weight: 1.1,
        high_weight: 0.7,
        lp_hz: 140.0,
        bass_ratio_min: 0.55,
        floor_mul_base: 3.2,
        floor_mul_sens_span: 3.5,
        cooldown_sens_span_ms: 60.0,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapperSettings {
    pub min_interval_ms: f32,
    /// When true, min_interval scales with sensitivity (house behavior).
    pub scale_interval_with_sensitivity: bool,
    pub intensity_floor: u8,
    pub intensity_ceil: u8,
    pub drop_hats: bool,
    /// boosted = strength * (strength_scale_base + sensitivity * strength_scale_sens)
    pub strength_scale_base: f32,
    pub strength_scale_sens: f32,
    /// Gate: boosted < min_strength_base + (1-sens)*min_strength_sens_span → drop
    pub min_strength_base: f32,
    pub min_strength_sens_span: f32,
    pub kick_strong_at: f32,
    pub kick_tick_at: f32,
    pub use_compound_kick: bool,
    /// Prefer pulse-type dynamics; still reports intensity for HID set_haptic.
    pub pulse_dynamics: bool,
}

impl Default for MapperSettings {
    fn default() -> Self {
        classic_mapper()
    }
}

fn classic_mapper() -> MapperSettings {
    MapperSettings {
        min_interval_ms: 70.0,
        scale_interval_with_sensitivity: false,
        // MX4 barely responds below ~50; old floor of 20 made Hits count with no feel.
        intensity_floor: 60,
        intensity_ceil: 100,
        drop_hats: false,
        strength_scale_base: 0.5,
        strength_scale_sens: 1.0,
        min_strength_base: 0.08,
        min_strength_sens_span: 0.0,
        kick_strong_at: 0.55,
        kick_tick_at: 0.25,
        use_compound_kick: true,
        pulse_dynamics: false,
    }
}

fn house_mapper() -> MapperSettings {
    MapperSettings {
        min_interval_ms: 110.0,
        scale_interval_with_sensitivity: true,
        intensity_floor: 40,
        intensity_ceil: 95,
        drop_hats: true,
        strength_scale_base: 0.35,
        strength_scale_sens: 0.9,
        min_strength_base: 0.18,
        min_strength_sens_span: 0.22,
        kick_strong_at: 0.72,
        kick_tick_at: 0.4,
        use_compound_kick: false,
        pulse_dynamics: true,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    pub drain_frames: usize,
    pub idle_sleep_ms: u64,
    /// Update HID intensity every N hits (1 = every hit).
    pub intensity_update_every_hits: u32,
    /// Also update when |delta intensity| exceeds this.
    pub intensity_delta_threshold: u8,
    pub base_intensity: u8,
}

impl Default for EngineSettings {
    fn default() -> Self {
        classic_engine()
    }
}

fn classic_engine() -> EngineSettings {
    EngineSettings {
        // Keep low buffering even for Classic — large drains felt "dead" on music.
        drain_frames: 512,
        idle_sleep_ms: 1,
        intensity_update_every_hits: 1,
        intensity_delta_threshold: 0,
        base_intensity: 75,
    }
}

fn house_engine() -> EngineSettings {
    EngineSettings {
        drain_frames: 512,
        idle_sleep_ms: 1,
        intensity_update_every_hits: 8,
        intensity_delta_threshold: 25,
        base_intensity: 70,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrumsConfig {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub sensitivity: f32,
    pub detector: DetectorSettings,
    pub mapper: MapperSettings,
    pub engine: EngineSettings,
}

impl Default for DrumsConfig {
    fn default() -> Self {
        Self::classic()
    }
}

impl DrumsConfig {
    pub fn classic() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            id: "classic".into(),
            name: "Classic".into(),
            description: "Original multi-band onset detector (pre-house tuning).".into(),
            sensitivity: 0.65,
            detector: classic_detector(),
            mapper: classic_mapper(),
            engine: classic_engine(),
        }
    }

    pub fn house() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            id: "house".into(),
            name: "House / Electronic".into(),
            description: "Kick-focused detector; drops hats; lower latency drains.".into(),
            sensitivity: 0.55,
            detector: house_detector(),
            mapper: house_mapper(),
            engine: house_engine(),
        }
    }

    pub fn builtins() -> Vec<Self> {
        vec![Self::classic(), Self::house()]
    }

    pub fn sanitize(mut self) -> Self {
        self.schema = SCHEMA_VERSION;
        self.sensitivity = self.sensitivity.clamp(0.05, 1.0);
        self.detector.cooldown_ms = self.detector.cooldown_ms.clamp(20.0, 400.0);
        self.detector.history_len = self.detector.history_len.clamp(8, 256);
        self.detector.lp_hz = self.detector.lp_hz.clamp(40.0, 400.0);
        self.detector.bass_ratio_min = self.detector.bass_ratio_min.clamp(0.0, 2.0);
        self.mapper.min_interval_ms = self.mapper.min_interval_ms.clamp(20.0, 500.0);
        // Intensities below ~50 are effectively silent on MX Master 4.
        self.mapper.intensity_floor = self.mapper.intensity_floor.max(50).min(100);
        if self.mapper.intensity_floor > self.mapper.intensity_ceil {
            std::mem::swap(
                &mut self.mapper.intensity_floor,
                &mut self.mapper.intensity_ceil,
            );
        }
        self.mapper.intensity_ceil = self.mapper.intensity_ceil.max(self.mapper.intensity_floor).min(100);
        self.engine.drain_frames = self.engine.drain_frames.clamp(128, 16_384);
        self.engine.idle_sleep_ms = self.engine.idle_sleep_ms.clamp(0, 50);
        self.engine.intensity_update_every_hits =
            self.engine.intensity_update_every_hits.clamp(1, 64);
        self.engine.base_intensity = self.engine.base_intensity.max(55).min(100);
        if self.id.trim().is_empty() {
            self.id = slugify(&self.name);
        }
        if self.name.trim().is_empty() {
            self.name = self.id.clone();
        }
        self
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        let cfg: DrumsConfig = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Ok(cfg.sanitize())
    }

    pub fn load_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json(&text)
    }

    pub fn save_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, self.to_json_pretty()?).map_err(|e| e.to_string())
    }
}

fn slugify(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "custom".into()
    } else {
        s
    }
}

pub fn app_support_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library/Application Support/MusicDrums")
}

pub fn presets_dir() -> PathBuf {
    app_support_dir().join("presets")
}

pub fn active_config_path() -> PathBuf {
    app_support_dir().join("active.json")
}

pub fn ensure_preset_dirs() -> Result<(), String> {
    fs::create_dir_all(presets_dir()).map_err(|e| e.to_string())
}

/// Builtin + user presets from Application Support.
pub fn list_presets() -> Vec<PresetInfo> {
    let mut out: Vec<PresetInfo> = DrumsConfig::builtins()
        .into_iter()
        .map(|c| PresetInfo {
            id: c.id.clone(),
            name: c.name.clone(),
            description: c.description.clone(),
            builtin: true,
            path: None,
        })
        .collect();

    if let Ok(entries) = fs::read_dir(presets_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(cfg) = DrumsConfig::load_file(&path) {
                out.push(PresetInfo {
                    id: cfg.id,
                    name: cfg.name,
                    description: cfg.description,
                    builtin: false,
                    path: Some(path.display().to_string()),
                });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub path: Option<String>,
}

pub fn load_preset_by_id(id: &str) -> Result<DrumsConfig, String> {
    if let Some(b) = DrumsConfig::builtins().into_iter().find(|c| c.id == id) {
        return Ok(b);
    }
    let path = presets_dir().join(format!("{id}.json"));
    if path.is_file() {
        return DrumsConfig::load_file(&path);
    }
    // Also accept absolute / relative file paths as "id".
    let as_path = PathBuf::from(id);
    if as_path.is_file() {
        return DrumsConfig::load_file(&as_path);
    }
    Err(format!("preset not found: {id}"))
}

pub fn save_user_preset(cfg: &DrumsConfig) -> Result<PathBuf, String> {
    ensure_preset_dirs()?;
    let path = presets_dir().join(format!("{}.json", cfg.id));
    cfg.save_file(&path)?;
    Ok(path)
}

pub fn load_active_or_default() -> DrumsConfig {
    let path = active_config_path();
    if path.is_file() {
        if let Ok(cfg) = DrumsConfig::load_file(&path) {
            return cfg;
        }
    }
    DrumsConfig::classic()
}

pub fn persist_active(cfg: &DrumsConfig) -> Result<(), String> {
    ensure_preset_dirs()?;
    cfg.save_file(&active_config_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_classic() {
        let j = DrumsConfig::classic().to_json_pretty().unwrap();
        let back = DrumsConfig::from_json(&j).unwrap();
        assert_eq!(back.id, "classic");
        assert!(matches!(back.detector.mode, DetectorMode::Multiband));
    }

    #[test]
    fn house_is_kick_mode() {
        assert!(matches!(
            DrumsConfig::house().detector.mode,
            DetectorMode::Kick
        ));
        assert!(DrumsConfig::house().mapper.drop_hats);
    }
}

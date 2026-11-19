//! Automation: lanes, points, curves for parameter automation.

use crate::mixer::MixerParam;
use serde::{Deserialize, Serialize};

/// Parameter that can be automated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationParam {
    /// Mixer channel parameter
    Mixer(MixerParam),
    /// Plugin parameter (insert index, param index)
    Plugin {
        insert_index: usize,
        param_index: u32,
    },
}

impl AutomationParam {
    pub fn mixer(param: MixerParam) -> Self {
        Self::Mixer(param)
    }

    pub fn plugin(insert_index: usize, param_index: u32) -> Self {
        Self::Plugin { insert_index, param_index }
    }

    pub fn display_name(&self) -> String {
        match self {
            AutomationParam::Mixer(p) => p.as_str().to_string(),
            AutomationParam::Plugin { insert_index, param_index } => {
                format!("Plugin{}Param{}", insert_index, param_index)
            }
        }
    }
}

/// Interpolation type between automation points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationType {
    /// Linear interpolation (straight line)
    Linear,
    /// Step/hold (value changes instantly at next point)
    Step,
    /// Bezier curve (smooth)
    Bezier,
}

/// A single automation point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    /// Position in beats
    pub beat: f32,
    /// Normalized value 0.0..1.0
    pub value: f32,
    /// Interpolation to next point
    pub interpolation: InterpolationType,
}

impl AutomationPoint {
    pub fn new(beat: f32, value: f32) -> Self {
        Self {
            beat,
            value: value.clamp(0.0, 1.0),
            interpolation: InterpolationType::Linear,
        }
    }
}

/// Automation curve for a single parameter on a track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationCurve {
    /// Parameter being automated
    pub param: AutomationParam,
    /// Automation points (sorted by beat)
    pub points: Vec<AutomationPoint>,
    /// Whether this lane is visible in the UI
    pub visible: bool,
    /// Lane color (palette index)
    pub color: u8,
}

impl AutomationCurve {
    pub fn new(param: AutomationParam) -> Self {
        Self {
            param,
            points: Vec::new(),
            visible: true,
            color: 0,
        }
    }

    /// Add a point, maintaining sort order by beat.
    pub fn add_point(&mut self, point: AutomationPoint) {
        let idx = self.points.partition_point(|p| p.beat < point.beat);
        self.points.insert(idx, point);
    }

    /// Remove point at index.
    pub fn remove_point(&mut self, index: usize) -> bool {
        if index < self.points.len() {
            self.points.remove(index);
            true
        } else {
            false
        }
    }

    /// Get interpolated value at a specific beat.
    pub fn value_at(&self, beat: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }

        // Before first point
        if beat <= self.points[0].beat {
            return self.points[0].value;
        }

        // After last point
        if beat >= self.points.last().unwrap().beat {
            return self.points.last().unwrap().value;
        }

        // Find surrounding points
        for i in 0..self.points.len() - 1 {
            let p1 = self.points[i];
            let p2 = self.points[i + 1];

            if beat >= p1.beat && beat <= p2.beat {
                let t = if p2.beat == p1.beat {
                    0.0
                } else {
                    (beat - p1.beat) / (p2.beat - p1.beat)
                };

                return match p1.interpolation {
                    InterpolationType::Linear => p1.value + (p2.value - p1.value) * t,
                    InterpolationType::Step => p1.value,
                    InterpolationType::Bezier => {
                        // Simplified: use linear for now
                        p1.value + (p2.value - p1.value) * t
                    }
                };
            }
        }

        0.0
    }
}

/// Collection of automation curves for a track.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AutomationLanes {
    pub curves: Vec<AutomationCurve>,
}

impl AutomationLanes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_curve(&mut self, curve: AutomationCurve) {
        self.curves.push(curve);
    }

    pub fn get_curve(&self, param: &AutomationParam) -> Option<&AutomationCurve> {
        self.curves.iter().find(|c| &c.param == param)
    }

    pub fn get_curve_mut(&mut self, param: &AutomationParam) -> Option<&mut AutomationCurve> {
        self.curves.iter_mut().find(|c| &c.param == param)
    }

    pub fn remove_curve(&mut self, param: &AutomationParam) -> bool {
        if let Some(idx) = self.curves.iter().position(|c| &c.param == param) {
            self.curves.remove(idx);
            true
        } else {
            false
        }
    }
}
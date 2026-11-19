//! Automation Lanes: faixas de automação dedicadas abaixo de cada track header.
//!
//! Mostra a curva de automação (volume, pan, plugin params) ao longo da timeline
//! e permite editar pontos diretamente na arrange window.

pub mod data;
pub mod interaction;
pub mod render;

pub use data::{AutomationLaneMetrics, AutomationPointView, AUTOMATION_LANE_HEIGHT, AUTOMATION_LANE_GAP};
pub use interaction::AutomationInteraction;
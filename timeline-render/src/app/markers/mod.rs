//! Marcadores na régua da timeline: posição, regiões, loops, CD tracks.
//!
//! Suporta:
//! - Marcadores de posição (pontos únicos)
//! - Regiões (início + fim, com preenchimento visual)
//! - Loops (regiões de repetição)
//! - Marcadores CD (faixas de CD Red Book)

pub mod data;
pub mod interaction;
pub mod render;

pub use data::{MarkerMetrics, build_all_marker_instances, build_marker_instances, build_region_instances};
pub use interaction::{MarkerDragMode, MarkerInteraction, hit_test_marker};
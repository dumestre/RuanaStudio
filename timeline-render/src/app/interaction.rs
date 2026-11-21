//! InteraÃ§Ã£o: mouse, scroll, ferramentas, clipboard, loop, foco e edge-scroll.

use crate::app::markers;
use crate::app::{App, ButtonState, KeyAction, LoopDrag};
use crate::clips;
use crate::fades;
use crate::tools::{
    Tool, current_tool, set_cursor_override, set_gain_override, set_hand_override, set_tool,
    set_vresize_override,
};
use daw_core::WarpMarker;
use std::time::Instant;

#[cfg(test)]
mod double_click_tests {
    use super::*;

    fn press(state: &mut DoubleClickState, at_ms: u64, pos: (f32, f32), clip: usize) -> bool {
        let now = Instant::now() + std::time::Duration::from_millis(at_ms);
        double_click_hit(state, now, pos, clip)
    }

    #[test]
    fn primeiro_clique_nao_e_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 0));
    }

    #[test]
    fn dois_cliques_no_mesmo_clip_sao_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(press(&mut s, 120, (11.0, 10.5), 3));
    }

    #[test]
    fn clip_diferente_nao_e_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(!press(&mut s, 100, (10.0, 10.0), 4));
    }

    #[test]
    fn longe_demais_nao_e_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(!press(&mut s, 100, (400.0, 10.0), 3));
    }

    #[test]
    fn longe_na_vertical_nao_e_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(!press(&mut s, 100, (10.0, 300.0), 3));
    }

    #[test]
    fn fora_da_janela_de_tempo_nao_e_duplo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(!press(&mut s, 900, (10.0, 10.0), 3));
    }

    #[test]
    fn terceiro_clique_nao_reabre() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(press(&mut s, 100, (10.0, 10.0), 3));
        // Logo em seguida não pode virar outro duplo clique.
        assert!(!press(&mut s, 150, (10.0, 10.0), 3));
    }

    #[test]
    fn segundo_duplo_clique_precisa_de_press_novo() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        assert!(press(&mut s, 100, (10.0, 10.0), 3));
        // O 3º clique guarda estado; o 4º fecha um NOVO duplo clique.
        assert!(!press(&mut s, 600, (10.0, 10.0), 3));
        assert!(press(&mut s, 700, (10.0, 10.0), 3));
    }

    #[test]
    fn clique_no_vazio_zera_o_estado() {
        let mut s: DoubleClickState = None;
        assert!(!press(&mut s, 0, (10.0, 10.0), 3));
        s = None;
        assert!(!press(&mut s, 100, (10.0, 10.0), 3));
    }
}

/// Estado do detector de duplo clique: último press (instante, posição, índice
/// do clip). `None` quando não há press pendente.
pub type DoubleClickState = Option<(Instant, (f32, f32), Option<usize>)>;

/// Janela de tempo e tolerância em pixels para considerar dois presses um
/// duplo clique.
const DOUBLE_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);
const DOUBLE_CLICK_SLOP_PX: f32 = 5.0;

/// Consome um press e devolve `true` se ele fecha um duplo clique no MESMO
/// clip (mesmo índice, dentro da janela de tempo, perto o bastante em px).
///
/// O estado é zerado ao detectar o duplo clique: assim um terceiro clique não
/// reabre o editor, e um clique simples posterior recomeça a contagem.
pub fn double_click_hit(
    state: &mut DoubleClickState,
    now: Instant,
    pos: (f32, f32),
    clip: usize,
) -> bool {
    let is_double = match *state {
        Some((at, prev, prev_clip)) => {
            now.duration_since(at) <= DOUBLE_CLICK_WINDOW
                && (pos.0 - prev.0).abs() <= DOUBLE_CLICK_SLOP_PX
                && (pos.1 - prev.1).abs() <= DOUBLE_CLICK_SLOP_PX
                && prev_clip == Some(clip)
        }
        None => false,
    };
    *state = if is_double {
        None
    } else {
        Some((now, pos, Some(clip)))
    };
    is_double
}

mod editing;
mod keyboard;
mod mouse;
mod piano_roll;
pub(super) mod pointer;
mod timewarp;
pub use piano_roll::{timeline_beat_at_x, timeline_warp_beat};

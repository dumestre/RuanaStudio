//! Normalização automática do ganho de clips importados.
//!
//! O objetivo é um PREMIX: todo áudio que entra na timeline nasce com
//! um nível parecido, tanto no ouvido quanto na waveform. O ganho é o
//! do CLIP (`Clip::gain_db`), aplicado antes da trilha e do mixer —
//! baixar/subir o fader da trilha continua independente disto.
//!
//! # Curva
//!
//! A normalização é por loudness aproximada (RMS do trecho) com um
//! teto de pico. Medir só o pico deixaria material muito dinâmico
//! (transientes altos, corpo baixo) mais baixo que o resto; medir só o
//! RMS deixaria o pico estourar. O ganho escolhido é o MENOR entre:
//!
//! - o que leva o RMS ao alvo ([`TARGET_RMS_DB`]);
//! - o que leva o pico ao teto ([`TARGET_PEAK_DB`]).
//!
//! Assim o material já alto é atenuado, o material baixo é reforçado,
//! e nenhum import passa do teto de pico. A janela é o próprio trecho
//! audível (o import já recorta o silêncio das bordas), então o número
//! reflete o que o usuário vai ouvir, não o arquivo cru.
//!
//! O RMS é um proxy barato de LUFS: sem o filtro K-weighting e o
//! gating do EBU R128, mas com a mesma ideia de "energia média". Para
//! um premix de DAW a diferença fica dentro da folga do fader.

/// Loudness alvo do premix, em dBFS RMS.
///
/// -18 dBFS é o ponto clássico de gain staging de DAW: sobra headroom
/// para somar faixas sem o master estourar.
pub const TARGET_RMS_DB: f32 = -18.0;

/// Teto de pico do material normalizado, em dBFS.
///
/// -1 dBFS deixa folga para o limitador/encode sem perder transiente.
pub const TARGET_PEAK_DB: f32 = -1.0;

/// Faixa de ganho aceita (a mesma do fader de clip no core).
pub const MIN_GAIN_DB: f32 = -24.0;
pub const MAX_GAIN_DB: f32 = 12.0;

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Pico e RMS (linear) de um trecho mono.
///
/// Ignora amostras não finitas para um NaN não contaminar o ganho.
pub fn peak_and_rms(samples: &[f32]) -> (f32, f32) {
    let mut peak = 0.0f32;
    let mut energy = 0.0f64;
    let mut count = 0usize;
    for &s in samples {
        if !s.is_finite() {
            continue;
        }
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        energy += (s as f64) * (s as f64);
        count += 1;
    }
    let rms = if count > 0 {
        (energy / count as f64).sqrt() as f32
    } else {
        0.0
    };
    (peak, rms)
}

/// Ganho de normalização, em dB, para um trecho mono.
///
/// Silêncio (ou trecho vazio) devolve `0.0`: não há o que normalizar e
/// um boost infinito seria absurdo.
pub fn gain_db_for(samples: &[f32]) -> f32 {
    let (peak, rms) = peak_and_rms(samples);
    if peak <= 1e-6 || rms <= 1e-6 {
        return 0.0;
    }

    let rms_gain = db_to_linear(TARGET_RMS_DB) / rms;
    let peak_gain = db_to_linear(TARGET_PEAK_DB) / peak;
    let gain = rms_gain.min(peak_gain);

    let gain_db = 20.0 * gain.log10();
    if gain_db.is_finite() {
        gain_db.clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_stays_untouched() {
        assert_eq!(gain_db_for(&[]), 0.0);
        assert_eq!(gain_db_for(&[0.0; 1024]), 0.0);
    }

    #[test]
    fn quiet_material_is_boosted_but_capped_by_peak() {
        // Senoide de amplitude 0.1: RMS ~0.0707, pico 0.1.
        let samples: Vec<f32> = (0..48_000)
            .map(|i| 0.1 * (i as f32 * 0.05).sin())
            .collect();
        let gain = gain_db_for(&samples);
        // O RMS pediria ~+4 dB; o pico permite bem mais. Vale o RMS.
        assert!((gain - 4.03).abs() < 0.2, "ganho inesperado: {gain}");
    }

    #[test]
    fn loud_material_is_attenuated() {
        let samples: Vec<f32> = (0..48_000)
            .map(|i| 0.9 * (i as f32 * 0.05).sin())
            .collect();
        let gain = gain_db_for(&samples);
        assert!(gain < 0.0, "material alto tem que descer: {gain}");
    }

    #[test]
    fn peak_ceiling_wins_for_a_lone_transient() {
        // Corpo baixíssimo + um pico em 0.99: o RMS pediria muito
        // ganho, mas o pico só deixa ~-1 dB.
        let mut samples = vec![0.001f32; 48_000];
        samples[100] = 0.99;
        let gain = gain_db_for(&samples);
        let peak_after = 0.99 * db_to_linear(gain);
        assert!(
            peak_after <= db_to_linear(TARGET_PEAK_DB) + 1e-4,
            "pico passou do teto: {peak_after}"
        );
    }

    #[test]
    fn gain_stays_inside_the_clip_fader_range() {
        for amplitude in [0.0001f32, 0.01, 0.5, 1.0] {
            let samples: Vec<f32> = (0..4096)
                .map(|i| amplitude * (i as f32 * 0.05).sin())
                .collect();
            let gain = gain_db_for(&samples);
            assert!((MIN_GAIN_DB..=MAX_GAIN_DB).contains(&gain), "fora: {gain}");
        }
    }
}

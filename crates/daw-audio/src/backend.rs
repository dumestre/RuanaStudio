use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

/// Erros do audio backend.
#[derive(Debug)]
pub enum AudioError {
    NoDefaultDevice,
    NoSupportedConfig,
    BuildStream(cpal::Error),
    PlayStream(cpal::Error),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::NoDefaultDevice => write!(f, "nenhum dispositivo de áudio encontrado"),
            AudioError::NoSupportedConfig => write!(f, "configuração de áudio não suportada"),
            AudioError::BuildStream(e) => write!(f, "erro ao criar stream: {}", e),
            AudioError::PlayStream(e) => write!(f, "erro ao iniciar stream: {}", e),
        }
    }
}

impl std::error::Error for AudioError {}

/// Configuração do audio backend.
pub struct AudioConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub channels: u16,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 2048,
            channels: 2,
        }
    }
}

/// Buffers pequenos (< 512) fazem a thread RT de áudio derrubar amostras
/// quando a CPU também está renderizando a UI (Skia + wgpu). O padrão do
/// `BufferSize::Default` varia muito entre hosts (WASAPI costuma dar ~10ms,
/// mas hosts "low-latency" podem dar menos que 2ms) e o `cpal` não expõe o
/// valor real — daí a escolha explícita de um buffer grande aqui.
const MIN_BUFFER_FRAMES: u32 = 1024;

/// Envia a configuração real que chegou a cpal para o log de diagnóstico.
/// Isso mostra qual `BufferSize` o device aceitou e serve de base para
/// debugar buffer underrun quando o app "granula" (buffer baixo).
fn log_stream_config(sample_rate: u32, buffer_size: cpal::BufferSize, channels: u16) {
    if std::env::var("RUANA_LOG").is_ok() {
        let ms = match buffer_size {
            cpal::BufferSize::Fixed(n) => n as f64 / sample_rate as f64 * 1000.0,
            cpal::BufferSize::Default => {
                eprintln!(
                    "[backend] BUFFER DEFAULT: o OS decidiu o buffer. Configure um valor fixo em `default_output_config` para estabilidade."
                );
                0.0
            }
        };
        eprintln!(
            "[backend] stream: {} Hz / {:?}/ frames (~{:.1} ms) / {} canais",
            sample_rate, buffer_size, ms, channels
        );
    }
}

/// Escolhe a melhor configuração de saída suportada pelo dispositivo.
///
/// Estratégia: um buffer de pelo menos [`MIN_BUFFER_FRAMES`] frames é
/// prioridade, porque o app renderiza a UI ao mesmo tempo que toca áudio.
/// Sem isso o RT do cpal estoura ("granulado", "falhas", "buffer baixo").
/// O fallback é o `default_output_config()` — que o `cpal` mantém em
/// `BufferSize::Default` e o `CpalBackend::new` traduz para um valor fixo.
fn pick_supported_config(
    device: &cpal::Device,
) -> Result<(cpal::StreamConfig, cpal::SampleFormat), AudioError> {
    let default = device
        .default_output_config()
        .map_err(|_| AudioError::NoSupportedConfig)?;

    // A taxa padrão do dispositivo (o motor assume 48 kHz, mas o cpal
    // pode devolver outra — ex.: 44.1 kHz em drivers baratos).
    let want_rate = default.sample_rate();

    for range in device
        .supported_output_configs()
        .map_err(|_| AudioError::NoSupportedConfig)?
    {
        if range.channels() != default.channels() {
            continue;
        }
        if range.max_sample_rate() < want_rate || range.min_sample_rate() > want_rate {
            continue;
        }
        // O buffer pode ser `Range { min, max }` ou `Unknown`.
        let max_buf = match range.buffer_size() {
            cpal::SupportedBufferSize::Range { max, .. } => *max,
            cpal::SupportedBufferSize::Unknown => 0,
        };
        if max_buf >= MIN_BUFFER_FRAMES {
            let candidate = range.with_sample_rate(want_rate);
            let format = candidate.sample_format();
            // Fixa o tamanho do buffer ANTES de enviar para cpal.
            // O `BufferSize::Default` passa a ser o OS quem decide — e
            // pode voltar a ser pequeno de novo ("buffer baixo").
            let conf: cpal::StreamConfig = candidate.into();
            let buffer_size = if cfg!(target_os = "linux") {
                cpal::BufferSize::Default
            } else {
                cpal::BufferSize::Fixed(MIN_BUFFER_FRAMES)
            };
            return Ok((
                cpal::StreamConfig {
                    channels: conf.channels,
                    sample_rate: conf.sample_rate,
                    buffer_size,
                },
                format,
            ));
        }
    }

    let format = default.sample_format();
    // O PipeWire/PulseAudio define o buffer real no Linux; impor 1024
    // frames pode rejeitar a configuração virtual padrão exposta pelo CPAL.
    let mut stream_config: cpal::StreamConfig = default.into();
    if !cfg!(target_os = "linux") {
        stream_config.buffer_size = cpal::BufferSize::Fixed(MIN_BUFFER_FRAMES);
    }
    Ok((stream_config, format))
}

/// Backend CPAL — abre a stream de saída de áudio do sistema.
pub struct CpalBackend {
    _stream: Stream,
    config: AudioConfig,
}

impl CpalBackend {
    /// Cria e inicia o backend CPAL com o callback dado.
    /// O callback recebe um slice f32 stereo intercalado (L, R, L, R, ...).
    pub fn new(mut callback: impl FnMut(&mut [f32]) + Send + 'static) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoDefaultDevice)?;

        let (config, sample_format) = pick_supported_config(&device)?;

        let sample_rate = config.sample_rate;
        let channels = config.channels as u16;
        let buffer_size = match &config.buffer_size {
            // pick_supported_config já fixa esse valor, mas o fallback
            // (default do OS) segue aqui como backup.
            cpal::BufferSize::Default => 1024,
            cpal::BufferSize::Fixed(n) => *n,
        };

        log_stream_config(sample_rate, config.buffer_size, channels);
        eprintln!("[backend] sample format: {:?}", sample_format);

        let config_info = AudioConfig {
            sample_rate,
            buffer_size,
            channels,
        };

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                config.clone(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    callback(data);
                },
                |err| eprintln!("audio stream error: {}", err),
                None,
            ),
            SampleFormat::I16 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::U16 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = ((s.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::I32 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = (s.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::U32 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [u32], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = ((s.clamp(-1.0, 1.0) + 1.0) * (u32::MAX as f32) * 0.5) as u32;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::F64 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [f64], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = s.clamp(-1.0, 1.0) as f64;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::I8 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [i8], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = (s.clamp(-1.0, 1.0) * i8::MAX as f32) as i8;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            SampleFormat::U8 => {
                let mut inner_callback = Some(callback);
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [u8], _: &cpal::OutputCallbackInfo| {
                        let cb = inner_callback.as_mut().unwrap();
                        let mut f32_buf = vec![0.0f32; data.len()];
                        cb(&mut f32_buf);
                        for (out, &s) in data.iter_mut().zip(f32_buf.iter()) {
                            *out = ((s.clamp(-1.0, 1.0) + 1.0) * 0.5 * u8::MAX as f32) as u8;
                        }
                    },
                    |err| eprintln!("audio stream error: {}", err),
                    None,
                )
            }
            _ => return Err(AudioError::NoSupportedConfig),
        }
        .map_err(AudioError::BuildStream)?;

        stream.play().map_err(AudioError::PlayStream)?;

        Ok(Self {
            _stream: stream,
            config: config_info,
        })
    }

    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
}

pub fn available_output_hosts() -> Vec<String> {
    cpal::available_hosts()
        .into_iter()
        .map(|host| host.name().to_string())
        .collect()
}

pub fn output_device_names(host_name: &str) -> Vec<String> {
    let host = cpal::available_hosts()
        .into_iter()
        .find(|host| host.name().eq_ignore_ascii_case(host_name))
        .and_then(|host| cpal::host_from_id(host).ok())
        .unwrap_or_else(cpal::default_host);

    host.output_devices()
        .ok()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    let name = device.to_string();
                    (!name.is_empty()).then_some(name)
                })
                .collect()
        })
        .unwrap_or_default()
}

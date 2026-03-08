use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tauri::Emitter;

use crate::commands::audio::MicrophoneInfo;

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;
/// Emit waveform data every ~64ms (1024 samples at 16kHz)
const WAVEFORM_EMIT_SAMPLES: usize = 1024;
/// Number of amplitude bars the frontend expects
const WAVEFORM_BARS: usize = 24;
/// Minimum samples to consider a valid recording (~0.3s at 16kHz)
const MIN_RECORDING_SAMPLES: usize = 4800;
/// RMS threshold for silence detection — low enough for cheap/quiet microphones
const SILENCE_RMS_THRESHOLD: f64 = 50.0;

pub struct AudioRecorder {
    is_recording: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<i16>>>,
    app_handle: Option<tauri::AppHandle>,
    capture_thread: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            buffer: Arc::new(Mutex::new(Vec::new())),
            app_handle: None,
            capture_thread: None,
        }
    }

    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        self.app_handle = Some(handle);
    }

    pub fn start(&mut self, device_name: Option<String>) -> Result<(), AudioError> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyRecording);
        }

        // Normalize: None and "default" both mean system default
        let normalized = match device_name.as_deref() {
            None | Some("default") => None,
            Some(name) => Some(name.to_string()),
        };

        // Clear buffer for the new recording
        {
            let mut buf = self.buffer.lock().unwrap();
            buf.clear();
        }

        self.is_recording.store(true, Ordering::SeqCst);

        let is_recording = self.is_recording.clone();
        let buffer = self.buffer.clone();
        let app_handle = self.app_handle.clone();

        let capture_thread = std::thread::spawn(move || {
            if let Err(e) = capture_thread_main(normalized, is_recording.clone(), buffer, app_handle)
            {
                log::error!("[AudioRecorder] Capture thread error: {}", e);
                is_recording.store(false, Ordering::SeqCst);
            }
        });

        self.capture_thread = Some(capture_thread);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<Vec<u8>, AudioError> {
        if !self.is_recording.load(Ordering::SeqCst) {
            return Err(AudioError::NotRecording);
        }

        self.is_recording.store(false, Ordering::SeqCst);

        // Wait for capture thread to finish (it handles audio_client.Stop() + cleanup)
        if let Some(thread) = self.capture_thread.take() {
            let _ = thread.join();
        }

        let samples = {
            let buf = self.buffer.lock().unwrap();
            buf.clone()
        };

        if samples.len() < MIN_RECORDING_SAMPLES {
            return Err(AudioError::EmptyRecording);
        }

        if !has_speech(&samples) {
            return Err(AudioError::EmptyRecording);
        }

        encode_wav(&samples)
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

/// Main capture thread function — runs all WASAPI operations on a single thread.
/// COM is initialized here, and all COM objects are created and destroyed within this thread.
fn capture_thread_main(
    device_name: Option<String>,
    is_recording: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<i16>>>,
    app_handle: Option<tauri::AppHandle>,
) -> Result<(), AudioError> {
    use windows::core::Interface;
    use windows::Win32::Foundation::{HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::System::Threading::{CreateEventA, WaitForSingleObject};

    // PKEY_Device_FriendlyName: {a45c254e-df1c-4efd-8020-67d146a850e0}, pid 14
    let pkey_friendly_name = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    unsafe {
        // Initialize COM on this thread
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| AudioError::DeviceError(format!("CoInitializeEx failed: {}", e)))?;

        let _com_guard = ComUninitGuard;

        // Create device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| AudioError::DeviceError(format!("CoCreateInstance failed: {}", e)))?;

        // Resolve device
        let device = resolve_device(&enumerator, &device_name, &pkey_friendly_name)?;

        // Activate IAudioClient
        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| AudioError::DeviceError(format!("Activate IAudioClient failed: {}", e)))?;

        // Try to cast to IAudioClient2 and set Communications category
        if let Ok(audio_client2) = audio_client.cast::<IAudioClient2>() {
            let props = AudioClientProperties {
                cbSize: std::mem::size_of::<AudioClientProperties>() as u32,
                bIsOffload: false.into(),
                eCategory: AudioCategory_Communications,
                Options: AUDCLNT_STREAMOPTIONS_NONE,
            };
            if let Err(e) = audio_client2.SetClientProperties(&props) {
                log::warn!(
                    "[AudioRecorder] SetClientProperties(Communications) failed: {} — ducking won't work",
                    e
                );
            }
        } else {
            log::warn!("[AudioRecorder] IAudioClient2 not available — ducking won't work");
        }

        // Get device mix format
        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| AudioError::DeviceError(format!("GetMixFormat failed: {}", e)))?;
        let mix_format = &*mix_format_ptr;

        let device_sample_rate = mix_format.nSamplesPerSec;
        let device_channels = mix_format.nChannels;
        let device_bits = mix_format.wBitsPerSample;

        log::info!(
            "[AudioRecorder] Device format: {}Hz, {}ch, {}bit",
            device_sample_rate,
            device_channels,
            device_bits
        );

        // Buffer duration: 1 second in 100ns units
        let buffer_duration: i64 = 10_000_000;

        // Initialize audio client in shared mode with event callback
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                buffer_duration,
                0,
                mix_format_ptr,
                None,
            )
            .map_err(|e| AudioError::DeviceError(format!("Initialize failed: {}", e)))?;

        // Create event for buffer-ready notifications
        let event: HANDLE = CreateEventA(None, false, false, None)
            .map_err(|e| AudioError::DeviceError(format!("CreateEventA failed: {}", e)))?;

        audio_client
            .SetEventHandle(event)
            .map_err(|e| AudioError::DeviceError(format!("SetEventHandle failed: {}", e)))?;

        // Get capture client
        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| AudioError::DeviceError(format!("GetService failed: {}", e)))?;

        // Start capturing
        audio_client
            .Start()
            .map_err(|e| AudioError::DeviceError(format!("Start failed: {}", e)))?;

        log::info!("[AudioRecorder] WASAPI capture started");

        // Waveform state
        let mut amplitude_buf: Vec<f32> = Vec::new();
        let sample_counter = AtomicUsize::new(0);

        // Capture loop
        while is_recording.load(Ordering::SeqCst) {
            let wait_result = WaitForSingleObject(event, 200);
            if wait_result != WAIT_OBJECT_0 {
                continue;
            }

            // Process all available packets
            loop {
                let packet_size = capture_client
                    .GetNextPacketSize()
                    .map_err(|e| AudioError::DeviceError(format!("GetNextPacketSize: {}", e)))?;

                if packet_size == 0 {
                    break;
                }

                let mut data_ptr = std::ptr::null_mut();
                let mut num_frames = 0u32;
                let mut flags = 0u32;
                let mut device_position = 0u64;
                let mut qpc_position = 0u64;

                capture_client
                    .GetBuffer(
                        &mut data_ptr,
                        &mut num_frames,
                        &mut flags,
                        Some(&mut device_position),
                        Some(&mut qpc_position),
                    )
                    .map_err(|e| AudioError::DeviceError(format!("GetBuffer: {}", e)))?;

                let is_silent = (flags & 0x2) != 0; // AUDCLNT_BUFFERFLAGS_SILENT
                let total_samples = num_frames as usize * device_channels as usize;

                if !is_silent && total_samples > 0 {
                    // Convert device format to mono i16
                    let samples = if device_bits == 32 {
                        let float_slice =
                            std::slice::from_raw_parts(data_ptr as *const f32, total_samples);
                        convert_to_mono_i16_f32(float_slice, device_channels as usize)
                    } else if device_bits == 16 {
                        let i16_slice =
                            std::slice::from_raw_parts(data_ptr as *const i16, total_samples);
                        convert_to_mono_i16(i16_slice, device_channels as usize)
                    } else {
                        // Most WASAPI shared mode is float32
                        let float_slice =
                            std::slice::from_raw_parts(data_ptr as *const f32, total_samples);
                        convert_to_mono_i16_f32(float_slice, device_channels as usize)
                    };

                    // Compute waveform amplitude
                    let rms = if !samples.is_empty() {
                        let sum: f32 = samples
                            .iter()
                            .map(|&s| {
                                let f = s as f32 / i16::MAX as f32;
                                f * f
                            })
                            .sum();
                        (sum / samples.len() as f32).sqrt()
                    } else {
                        0.0
                    };
                    let scaled = (rms.sqrt() * 2.0).clamp(0.0, 1.0);
                    amplitude_buf.push(scaled);

                    // Resample to 16kHz if needed
                    let resampled = if device_sample_rate != SAMPLE_RATE {
                        resample(&samples, device_sample_rate, SAMPLE_RATE)
                    } else {
                        samples
                    };

                    // Push to buffer
                    {
                        let mut buf = buffer.lock().unwrap();
                        buf.extend_from_slice(&resampled);
                    }

                    // Emit waveform data periodically
                    let prev = sample_counter.load(Ordering::Relaxed);
                    let new_count = prev + resampled.len();
                    sample_counter.store(new_count, Ordering::Relaxed);

                    if prev / WAVEFORM_EMIT_SAMPLES < new_count / WAVEFORM_EMIT_SAMPLES {
                        if let Some(ref handle) = app_handle {
                            let amplitudes: Vec<f32> = if amplitude_buf.len() >= WAVEFORM_BARS {
                                amplitude_buf[amplitude_buf.len() - WAVEFORM_BARS..].to_vec()
                            } else {
                                let mut v = vec![0.0f32; WAVEFORM_BARS - amplitude_buf.len()];
                                v.extend_from_slice(&amplitude_buf);
                                v
                            };
                            let _ = handle.emit(
                                "waveform-data",
                                serde_json::json!({ "amplitudes": amplitudes }),
                            );
                            amplitude_buf.clear();
                        }
                    }
                }

                capture_client
                    .ReleaseBuffer(num_frames)
                    .map_err(|e| AudioError::DeviceError(format!("ReleaseBuffer: {}", e)))?;
            }
        }

        // Stop and cleanup
        let _ = audio_client.Stop();
        log::info!("[AudioRecorder] WASAPI capture stopped");

        // Free the mix format
        CoTaskMemFree(Some(mix_format_ptr as *const _ as *const std::ffi::c_void));

        // Close the event handle
        let _ = windows::Win32::Foundation::CloseHandle(event);
    }

    Ok(())
}

/// RAII guard to call CoUninitialize when dropped
struct ComUninitGuard;

impl Drop for ComUninitGuard {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}

/// Resolve an audio capture device by name, falling back to system default.
unsafe fn resolve_device(
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
    device_name: &Option<String>,
    pkey_friendly_name: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<windows::Win32::Media::Audio::IMMDevice, AudioError> {
    use windows::Win32::Media::Audio::*;

    match device_name {
        None => enumerator
            .GetDefaultAudioEndpoint(eCapture, eCommunications)
            .map_err(|_| AudioError::NoDevice),
        Some(name) => {
            let collection = enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .map_err(|e| AudioError::DeviceError(format!("EnumAudioEndpoints: {}", e)))?;

            let count = collection
                .GetCount()
                .map_err(|e| AudioError::DeviceError(format!("GetCount: {}", e)))?;

            for i in 0..count {
                if let Ok(device) = collection.Item(i) {
                    if let Ok(friendly) = get_device_friendly_name(&device, pkey_friendly_name) {
                        if friendly == *name {
                            return Ok(device);
                        }
                    }
                }
            }

            log::warn!(
                "Configured microphone '{}' not found, falling back to system default",
                name
            );
            enumerator
                .GetDefaultAudioEndpoint(eCapture, eCommunications)
                .map_err(|_| AudioError::NoDevice)
        }
    }
}

/// Extract the friendly name string from an IMMDevice using its property store.
unsafe fn get_device_friendly_name(
    device: &windows::Win32::Media::Audio::IMMDevice,
    pkey: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<String, AudioError> {
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::Win32::System::Com::{CoTaskMemFree, STGM_READ};

    let store = device
        .OpenPropertyStore(STGM_READ)
        .map_err(|e| AudioError::DeviceError(format!("OpenPropertyStore: {}", e)))?;

    let prop = store
        .GetValue(pkey)
        .map_err(|e| AudioError::DeviceError(format!("GetValue: {}", e)))?;

    let pwstr = PropVariantToStringAlloc(&prop)
        .map_err(|e| AudioError::DeviceError(format!("PropVariantToStringAlloc: {}", e)))?;

    let name = pwstr.to_string().unwrap_or_default();

    // Free the allocated string
    CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));

    Ok(name)
}

/// Convert multi-channel f32 samples to mono i16
fn convert_to_mono_i16_f32(samples: &[f32], channels: usize) -> Vec<i16> {
    samples
        .chunks(channels)
        .map(|frame| {
            let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
            (mono * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16
        })
        .collect()
}

/// Convert multi-channel i16 samples to mono i16
fn convert_to_mono_i16(samples: &[i16], channels: usize) -> Vec<i16> {
    if channels == 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|&s| s as i32).sum();
            (sum / channels as i32) as i16
        })
        .collect()
}

/// Simple linear resampling from source_rate to target_rate
fn resample(samples: &[i16], source_rate: u32, target_rate: u32) -> Vec<i16> {
    if source_rate == target_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = source_rate as f64 / target_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;

        let sample = if idx + 1 < samples.len() {
            let s0 = samples[idx] as f64;
            let s1 = samples[idx + 1] as f64;
            (s0 + frac * (s1 - s0)) as i16
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0
        };

        output.push(sample);
    }

    output
}

fn has_speech(samples: &[i16]) -> bool {
    if samples.is_empty() {
        return false;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt();
    rms >= SILENCE_RMS_THRESHOLD
}

fn encode_wav(samples: &[i16]) -> Result<Vec<u8>, AudioError> {
    let mut cursor = std::io::Cursor::new(Vec::new());

    let spec = hound::WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .map_err(|e| AudioError::EncodingError(e.to_string()))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| AudioError::EncodingError(e.to_string()))?;
    }

    writer
        .finalize()
        .map_err(|e| AudioError::EncodingError(e.to_string()))?;

    Ok(cursor.into_inner())
}

pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, AudioError> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    let pkey_friendly_name = windows::Win32::Foundation::PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| AudioError::DeviceError(format!("CoInitializeEx failed: {}", e)))?;

        let _com_guard = ComUninitGuard;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| AudioError::DeviceError(format!("CoCreateInstance failed: {}", e)))?;

        // Get default communication device name (matches what start() uses)
        let default_name = enumerator
            .GetDefaultAudioEndpoint(eCapture, eCommunications)
            .ok()
            .and_then(|device| get_device_friendly_name(&device, &pkey_friendly_name).ok())
            .unwrap_or_default();

        // Enumerate all active capture devices
        let collection = enumerator
            .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            .map_err(|e| AudioError::DeviceError(format!("EnumAudioEndpoints: {}", e)))?;

        let count = collection
            .GetCount()
            .map_err(|e| AudioError::DeviceError(format!("GetCount: {}", e)))?;

        let mut mics = Vec::new();

        for i in 0..count {
            if let Ok(device) = collection.Item(i) {
                if let Ok(name) = get_device_friendly_name(&device, &pkey_friendly_name) {
                    if !name.is_empty() {
                        mics.push(MicrophoneInfo {
                            is_default: name == default_name,
                            name,
                        });
                    }
                }
            }
        }

        Ok(mics)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Already recording")]
    AlreadyRecording,
    #[error("Not recording")]
    NotRecording,
    #[error("No input device found")]
    NoDevice,
    #[error("Empty recording")]
    EmptyRecording,
    #[error("Audio device error: {0}")]
    DeviceError(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
}

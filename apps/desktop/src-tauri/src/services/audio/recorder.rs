use std::sync::mpsc;
use std::thread::JoinHandle;
use tauri::Emitter;

use crate::commands::audio::MicrophoneInfo;
use crate::services::audio::device_monitor::DeviceSelection;

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

/// Commands sent from AudioRecorder to the persistent capture thread.
enum CaptureCmd {
    /// Begin capturing audio with the current device selection.
    /// Carries a fresh DeviceSelection so the thread uses the latest endpoint ID.
    Start(DeviceSelection),
    /// Stop capturing, return samples. Thread responds with Samples or Error.
    Stop,
    /// Release all COM resources and exit. Thread responds with ShutdownAck.
    Shutdown,
}

/// Results sent from the capture thread back to AudioRecorder.
enum CaptureResult {
    /// Capture loop is now running (or WASAPI init succeeded on thread start).
    Started,
    /// Audio data from a completed recording.
    Samples(Vec<i16>),
    /// A WASAPI/COM error occurred. String describes the error.
    /// Bool indicates whether the thread is still alive (true) or has exited (false).
    Error(String, bool),
    /// Thread has released COM resources and will exit.
    ShutdownAck,
}

pub struct AudioRecorder {
    /// Send commands to the capture thread.
    cmd_tx: Option<mpsc::Sender<CaptureCmd>>,
    /// Receive results from the capture thread.
    result_rx: Option<mpsc::Receiver<CaptureResult>>,
    /// Handle to the capture thread for joining on shutdown.
    capture_thread: Option<JoinHandle<()>>,
    /// Tauri app handle for waveform event emission (cloned into thread).
    app_handle: Option<tauri::AppHandle>,
    /// Track whether we're currently recording (for is_recording() API).
    recording: bool,
    /// The device name the current thread was initialized with.
    /// None if no thread is running. Used to detect device changes.
    current_device: Option<Option<String>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            cmd_tx: None,
            result_rx: None,
            capture_thread: None,
            app_handle: None,
            recording: false,
            current_device: None,
        }
    }

    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        self.app_handle = Some(handle);
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Ensure a capture thread is running for the given device.
    /// If the device identity changed, shuts down the old thread and spawns a new one.
    /// If no thread exists, spawns one and waits for WASAPI initialization.
    fn ensure_thread(&mut self, device: &DeviceSelection) -> Result<(), AudioError> {
        let identity = device_identity(device);
        // If thread exists for the same identity, nothing to do
        if self.cmd_tx.is_some() && self.current_device.as_ref() == Some(&identity) {
            return Ok(());
        }

        // Device changed or no thread — shut down existing thread if any
        self.shutdown_thread();

        // Spawn new capture thread
        let (cmd_tx, cmd_rx) = mpsc::channel::<CaptureCmd>();
        let (result_tx, result_rx) = mpsc::channel::<CaptureResult>();
        let device_for_thread = device.clone();
        let app_handle = self.app_handle.clone();

        log::info!(
            "[AudioRecorder] Spawning capture thread for device={:?}",
            device
        );

        let thread = std::thread::Builder::new()
            .name("wasapi-capture".into())
            .spawn(move || {
                capture_thread_main(device_for_thread, cmd_rx, result_tx, app_handle);
            })
            .map_err(|e| {
                AudioError::DeviceError(format!("Failed to spawn capture thread: {}", e))
            })?;

        self.capture_thread = Some(thread);
        self.cmd_tx = Some(cmd_tx);
        self.result_rx = Some(result_rx);
        self.current_device = Some(identity);

        // Wait for the thread to finish WASAPI initialization.
        // The thread sends either Started (init OK, waiting for Start cmd)
        // or Error (init failed, thread exiting).
        match self.result_rx.as_ref().unwrap().recv() {
            Ok(CaptureResult::Started) => {
                log::info!("[AudioRecorder] Capture thread initialized successfully");
                Ok(())
            }
            Ok(CaptureResult::Error(msg, _alive)) => {
                log::error!("[AudioRecorder] Capture thread init failed: {}", msg);
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(msg))
            }
            _ => {
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(
                    "Unexpected response from capture thread".into(),
                ))
            }
        }
    }

    /// Shut down the capture thread gracefully.
    fn shutdown_thread(&mut self) {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(CaptureCmd::Shutdown);
            // Wait for ack with timeout
            if let Some(rx) = self.result_rx.as_ref() {
                let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
            }
        }
        self.result_rx = None;
        if let Some(thread) = self.capture_thread.take() {
            let _ = thread.join();
        }
        self.current_device = None;
        self.recording = false;
    }

    /// Clean up after a thread that has already exited (error/crash).
    fn cleanup_dead_thread(&mut self) {
        self.cmd_tx = None;
        self.result_rx = None;
        if let Some(thread) = self.capture_thread.take() {
            let _ = thread.join();
        }
        self.current_device = None;
        self.recording = false;
    }

    pub fn start(&mut self, device: DeviceSelection) -> Result<(), AudioError> {
        if self.recording {
            return Err(AudioError::AlreadyRecording);
        }

        log::debug!("[AudioRecorder] start() device={:?}", device);

        // Ensure thread is running (lazy init or device change)
        self.ensure_thread(&device)?;

        // Send Start command
        let tx = self
            .cmd_tx
            .as_ref()
            .ok_or_else(|| AudioError::DeviceError("No capture thread available".into()))?;

        tx.send(CaptureCmd::Start(device)).map_err(|_| {
            self.cleanup_dead_thread();
            AudioError::DeviceError("Capture thread died unexpectedly".into())
        })?;

        // Wait for Started confirmation
        let rx = self.result_rx.as_ref().unwrap();
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(CaptureResult::Started) => {
                self.recording = true;
                Ok(())
            }
            Ok(CaptureResult::Error(msg, alive)) => {
                log::error!("[AudioRecorder] Start failed: {}", msg);
                if !alive {
                    self.cleanup_dead_thread();
                }
                Err(AudioError::DeviceError(msg))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                log::error!("[AudioRecorder] Start command timed out (10s)");
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(
                    "Start timed out — audio device unresponsive".into(),
                ))
            }
            _ => {
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(
                    "Unexpected response from capture thread".into(),
                ))
            }
        }
    }

    pub fn stop(&mut self) -> Result<Vec<u8>, AudioError> {
        if !self.recording {
            return Err(AudioError::NotRecording);
        }

        log::debug!("[AudioRecorder] stop() — sending Stop command");

        let tx = self.cmd_tx.as_ref().ok_or_else(|| {
            self.recording = false;
            AudioError::DeviceError("No capture thread available".into())
        })?;

        tx.send(CaptureCmd::Stop).map_err(|_| {
            self.cleanup_dead_thread();
            AudioError::DeviceError("Capture thread died unexpectedly".into())
        })?;

        self.recording = false;

        // Wait for Samples response
        let rx = self.result_rx.as_ref().unwrap();
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(CaptureResult::Samples(samples)) => {
                log::debug!(
                    "[AudioRecorder] Samples received: {} ({:.1}s at {}Hz)",
                    samples.len(),
                    samples.len() as f64 / SAMPLE_RATE as f64,
                    SAMPLE_RATE,
                );

                if samples.len() < MIN_RECORDING_SAMPLES {
                    return Err(AudioError::EmptyRecording);
                }

                let (speech_detected, peak_rms) = has_speech(&samples);
                log::debug!(
                    "[AudioRecorder] Speech detection — peak RMS: {:.1}, threshold: {:.1}, detected: {}",
                    peak_rms, SILENCE_RMS_THRESHOLD, speech_detected
                );

                if !speech_detected {
                    return Err(AudioError::EmptyRecording);
                }

                encode_wav(&samples)
            }
            Ok(CaptureResult::Error(msg, alive)) => {
                log::error!("[AudioRecorder] Stop failed: {}", msg);
                if !alive {
                    self.cleanup_dead_thread();
                }
                Err(AudioError::DeviceError(msg))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                log::error!("[AudioRecorder] Stop command timed out (5s)");
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(
                    "Stop timed out — audio device unresponsive".into(),
                ))
            }
            _ => {
                self.cleanup_dead_thread();
                Err(AudioError::DeviceError(
                    "Unexpected response from capture thread".into(),
                ))
            }
        }
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.shutdown_thread();
    }
}

// ─── RAII Guards ───────────────────────────────────────────────────────────

/// RAII guard to free COM-allocated memory when dropped
struct CoTaskMemGuard(*const std::ffi::c_void);

impl Drop for CoTaskMemGuard {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoTaskMemFree(Some(self.0));
        }
    }
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

// ─── Persistent Capture Thread ─────────────────────────────────────────────

/// Persistent capture thread entry point.
///
/// Initializes COM + WASAPI once, then enters a command loop:
///   - Start: Reset() + Start() → capture loop → waits for Stop
///   - Stop: Stop() → sends samples back → waits for next command
///   - Shutdown: releases everything, exits
///
/// On DEVICE_INVALIDATED errors, attempts one automatic re-initialization.
fn capture_thread_main(
    device: DeviceSelection,
    cmd_rx: mpsc::Receiver<CaptureCmd>,
    result_tx: mpsc::Sender<CaptureResult>,
    app_handle: Option<tauri::AppHandle>,
) {
    match capture_thread_inner(&device, &cmd_rx, &result_tx, &app_handle) {
        Ok(()) => log::info!("[AudioRecorder] Capture thread exiting cleanly"),
        Err(e) => {
            log::error!("[AudioRecorder] Capture thread fatal error: {}", e);
            let _ = result_tx.send(CaptureResult::Error(e.to_string(), false));
        }
    }
}

/// Inner function that returns Result for clean error propagation.
fn capture_thread_inner(
    device: &DeviceSelection,
    cmd_rx: &mpsc::Receiver<CaptureCmd>,
    result_tx: &mpsc::Sender<CaptureResult>,
    app_handle: &Option<tauri::AppHandle>,
) -> Result<(), AudioError> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::{IAudioCaptureClient, IAudioClient};
    use windows::Win32::System::Com::*;

    let pkey_friendly_name = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    unsafe {
        // --- ONE-TIME INITIALIZATION ---
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| AudioError::DeviceError(format!("CoInitializeEx failed: {}", e)))?;
        let _com_guard = ComUninitGuard;

        // Initial WASAPI init validates the device works.
        // Objects are wrapped in Option — released after each Stop for BT A2DP restoration.
        let (ac, cc, sr, ch, bits) = init_wasapi_device(device, &pkey_friendly_name)?;
        let mut wasapi: Option<(IAudioClient, IAudioCaptureClient, u32, u16, u16)> =
            Some((ac, cc, sr, ch, bits));

        // Signal that initialization succeeded — thread is ready for commands
        let _ = result_tx.send(CaptureResult::Started);

        // Reusable buffer — capacity grows to max recording size and stays there
        let mut sample_buffer: Vec<i16> = Vec::new();

        // --- COMMAND LOOP (0% CPU while waiting — blocked on recv) ---
        loop {
            let cmd = match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => {
                    // Sender dropped — AudioRecorder was dropped without Shutdown
                    log::debug!("[AudioRecorder] Command channel closed, exiting");
                    break;
                }
            };

            match cmd {
                CaptureCmd::Start(fresh_device) => {
                    // Re-init WASAPI if it was released after previous Stop.
                    // Use fresh_device (not spawn-time device) so we get the current endpoint.
                    if wasapi.is_none() {
                        log::debug!("[AudioRecorder] Re-initializing WASAPI for new recording...");
                        let t = std::time::Instant::now();
                        match init_wasapi_device(&fresh_device, &pkey_friendly_name) {
                            Ok(w) => {
                                log::info!(
                                    "[AudioRecorder] WASAPI re-initialized in {:?}",
                                    t.elapsed()
                                );
                                wasapi = Some(w);
                            }
                            Err(e) => {
                                log::error!("[AudioRecorder] WASAPI re-init failed: {}", e);
                                let _ =
                                    result_tx.send(CaptureResult::Error(e.to_string(), true));
                                continue;
                            }
                        }
                    }

                    let (ref audio_client, ref capture_client, sr, ch, bits) =
                        *wasapi.as_ref().unwrap();

                    // Try Start(), with one auto-retry on device invalidation
                    let start_result = start_capture(audio_client);
                    match start_result {
                        Ok(()) => {}
                        Err(e) if is_device_invalidated(&e) => {
                            log::warn!(
                                "[AudioRecorder] Device invalidated on Start, reinitializing..."
                            );
                            // Drop stale objects before reinit
                            wasapi = None;
                            match reinit_and_start(&fresh_device, &pkey_friendly_name) {
                                Ok((new_client, new_capture, new_sr, new_ch, new_bits)) => {
                                    let _ = result_tx.send(CaptureResult::Started);
                                    let should_exit = capture_loop(
                                        &new_client,
                                        &new_capture,
                                        new_sr,
                                        new_ch,
                                        new_bits,
                                        cmd_rx,
                                        result_tx,
                                        app_handle,
                                        &mut sample_buffer,
                                    );
                                    // Don't store — let it drop (release for BT A2DP)
                                    if should_exit {
                                        let _ = result_tx.send(CaptureResult::ShutdownAck);
                                        return Ok(());
                                    }
                                    continue;
                                }
                                Err(e2) => {
                                    log::error!("[AudioRecorder] Reinit failed: {}", e2);
                                    let _ = result_tx
                                        .send(CaptureResult::Error(e2.to_string(), false));
                                    return Err(e2);
                                }
                            }
                        }
                        Err(e) => {
                            let _ =
                                result_tx.send(CaptureResult::Error(e.to_string(), true));
                            continue;
                        }
                    }

                    let _ = result_tx.send(CaptureResult::Started);

                    // Run capture loop until Stop or Shutdown
                    let should_exit = capture_loop(
                        audio_client,
                        capture_client,
                        sr,
                        ch,
                        bits,
                        cmd_rx,
                        result_tx,
                        app_handle,
                        &mut sample_buffer,
                    );

                    // Release WASAPI objects immediately after Stop.
                    // This lets BT headphones switch back to A2DP (high-quality playback).
                    // COM stays initialized on this thread — only WASAPI objects are released.
                    log::debug!("[AudioRecorder] Releasing WASAPI objects (BT A2DP restore)");
                    wasapi = None;

                    if should_exit {
                        let _ = result_tx.send(CaptureResult::ShutdownAck);
                        return Ok(());
                    }
                }

                CaptureCmd::Stop => {
                    // Not recording — send empty samples
                    log::debug!(
                        "[AudioRecorder] Stop received but not recording, sending empty"
                    );
                    let _ = result_tx.send(CaptureResult::Samples(Vec::new()));
                }

                CaptureCmd::Shutdown => {
                    log::info!("[AudioRecorder] Shutdown command received");
                    if let Some((ref ac, _, _, _, _)) = wasapi {
                        let _ = ac.Stop();
                    }
                    // wasapi dropped on return — releases WASAPI objects
                    let _ = result_tx.send(CaptureResult::ShutdownAck);
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// Initialize WASAPI: resolve device, activate IAudioClient, Initialize(), get capture client.
unsafe fn init_wasapi_device(
    device: &DeviceSelection,
    pkey_friendly_name: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<
    (
        windows::Win32::Media::Audio::IAudioClient,
        windows::Win32::Media::Audio::IAudioCaptureClient,
        u32,
        u16,
        u16,
    ),
    AudioError,
> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
        .map_err(|e| AudioError::DeviceError(format!("CoCreateInstance failed: {}", e)))?;

    let mm_device = resolve_device(&enumerator, device, pkey_friendly_name)?;

    let audio_client: IAudioClient = mm_device
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| AudioError::DeviceError(format!("Activate IAudioClient failed: {}", e)))?;

    let mix_format_ptr = audio_client
        .GetMixFormat()
        .map_err(|e| AudioError::DeviceError(format!("GetMixFormat failed: {}", e)))?;
    let _mix_format_guard = CoTaskMemGuard(mix_format_ptr as *const _ as *const std::ffi::c_void);
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

    // Buffer duration: 1 second (in 100ns units) for polling mode
    let buffer_duration: i64 = 10_000_000;

    audio_client
        .Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            0,
            buffer_duration,
            0,
            mix_format_ptr,
            None,
        )
        .map_err(|e| AudioError::DeviceError(format!("Initialize failed: {}", e)))?;

    let capture_client: IAudioCaptureClient = audio_client
        .GetService()
        .map_err(|e| AudioError::DeviceError(format!("GetService failed: {}", e)))?;

    log::info!("[AudioRecorder] WASAPI initialized successfully (polling mode)");
    Ok((
        audio_client,
        capture_client,
        device_sample_rate,
        device_channels,
        device_bits,
    ))
}

/// Reset() + Start() the audio client for a new recording.
unsafe fn start_capture(
    audio_client: &windows::Win32::Media::Audio::IAudioClient,
) -> Result<(), AudioError> {
    audio_client
        .Reset()
        .map_err(|e| AudioError::DeviceError(format!("Reset failed: {}", e)))?;
    audio_client
        .Start()
        .map_err(|e| AudioError::DeviceError(format!("Start failed: {}", e)))?;
    Ok(())
}

/// Check if an AudioError was caused by device invalidation (BT disconnect, sleep/wake, etc.)
fn is_device_invalidated(err: &AudioError) -> bool {
    match err {
        AudioError::DeviceError(msg) => {
            // AUDCLNT_E_DEVICE_INVALIDATED = 0x88890004
            // AUDCLNT_E_SERVICE_NOT_RUNNING = 0x88890010
            msg.contains("0x88890004")
                || msg.contains("AUDCLNT_E_DEVICE_INVALIDATED")
                || msg.contains("0x88890010")
                || msg.contains("AUDCLNT_E_SERVICE_NOT_RUNNING")
        }
        _ => false,
    }
}

/// Check if a windows::core::Error is a device-invalidated HRESULT.
fn is_device_invalidated_hresult(e: &windows::core::Error) -> bool {
    let code = format!("{}", e);
    code.contains("0x88890004") || code.contains("0x88890010")
}

/// Re-initialize WASAPI from scratch (after device invalidation).
/// Caller must already be on the COM-initialized thread.
unsafe fn reinit_and_start(
    device: &DeviceSelection,
    pkey_friendly_name: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<
    (
        windows::Win32::Media::Audio::IAudioClient,
        windows::Win32::Media::Audio::IAudioCaptureClient,
        u32,
        u16,
        u16,
    ),
    AudioError,
> {
    log::info!("[AudioRecorder] Re-initializing WASAPI after device invalidation...");
    let (client, capture, sr, ch, bits) = init_wasapi_device(device, pkey_friendly_name)?;
    start_capture(&client)?;
    Ok((client, capture, sr, ch, bits))
}

/// Capture audio in a polling loop until Stop or Shutdown command.
/// Returns true if Shutdown was received (caller should exit thread).
/// Returns false if Stop was received (caller should wait for next command).
unsafe fn capture_loop(
    audio_client: &windows::Win32::Media::Audio::IAudioClient,
    capture_client: &windows::Win32::Media::Audio::IAudioCaptureClient,
    device_sample_rate: u32,
    device_channels: u16,
    device_bits: u16,
    cmd_rx: &mpsc::Receiver<CaptureCmd>,
    result_tx: &mpsc::Sender<CaptureResult>,
    app_handle: &Option<tauri::AppHandle>,
    sample_buffer: &mut Vec<i16>,
) -> bool {
    sample_buffer.clear();

    let mut amplitude_buf: Vec<f32> = Vec::new();
    let mut sample_count: usize = 0;
    let mut loop_iterations: u64 = 0;
    let mut total_packets: u64 = 0;
    let mut silent_packets: u64 = 0;
    let capture_start = std::time::Instant::now();

    loop {
        // Check for commands (non-blocking)
        match cmd_rx.try_recv() {
            Ok(CaptureCmd::Stop) => {
                let _ = audio_client.Stop();
                let elapsed = capture_start.elapsed();
                log::info!(
                    "[AudioRecorder] Capture stopped — {:.1}s, {} packets ({} silent), {} samples",
                    elapsed.as_secs_f64(),
                    total_packets,
                    silent_packets,
                    sample_buffer.len()
                );
                let samples = std::mem::take(sample_buffer);
                let _ = result_tx.send(CaptureResult::Samples(samples));
                return false;
            }
            Ok(CaptureCmd::Shutdown) => {
                let _ = audio_client.Stop();
                log::info!("[AudioRecorder] Shutdown during capture");
                let _ = result_tx.send(CaptureResult::ShutdownAck);
                return true;
            }
            Ok(CaptureCmd::Start(_)) => {
                log::warn!("[AudioRecorder] Ignoring duplicate Start command during capture");
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                let _ = audio_client.Stop();
                log::debug!("[AudioRecorder] Command channel closed during capture");
                return true;
            }
        }

        // Poll every 10ms — negligible CPU for short PTT recordings
        std::thread::sleep(std::time::Duration::from_millis(10));
        loop_iterations += 1;

        // Log stats every ~20 seconds
        if loop_iterations % 2000 == 0 {
            log::debug!(
                "[AudioRecorder] Capture loop: {} iterations, {} packets, {} silent, {:.1}s",
                loop_iterations,
                total_packets,
                silent_packets,
                capture_start.elapsed().as_secs_f64()
            );
        }

        // Drain all available packets
        loop {
            let packet_size = match capture_client.GetNextPacketSize() {
                Ok(size) => size,
                Err(e) => {
                    log::error!("[AudioRecorder] GetNextPacketSize error: {}", e);
                    let _ = audio_client.Stop();
                    let _ = result_tx.send(CaptureResult::Error(
                        format!("GetNextPacketSize: {}", e),
                        !is_device_invalidated_hresult(&e),
                    ));
                    return true;
                }
            };

            if packet_size == 0 {
                break;
            }

            let mut data_ptr = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;
            let mut device_position = 0u64;
            let mut qpc_position = 0u64;

            if let Err(e) = capture_client.GetBuffer(
                &mut data_ptr,
                &mut num_frames,
                &mut flags,
                Some(&mut device_position),
                Some(&mut qpc_position),
            ) {
                log::error!("[AudioRecorder] GetBuffer error: {}", e);
                let _ = audio_client.Stop();
                let _ = result_tx
                    .send(CaptureResult::Error(format!("GetBuffer: {}", e), false));
                return true;
            }

            let is_silent = (flags & 0x2) != 0; // AUDCLNT_BUFFERFLAGS_SILENT
            let total_samples = num_frames as usize * device_channels as usize;
            total_packets += 1;
            if is_silent {
                silent_packets += 1;
            }

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

                sample_buffer.extend_from_slice(&resampled);

                // Emit waveform data periodically
                let prev = sample_count;
                sample_count += resampled.len();

                if prev / WAVEFORM_EMIT_SAMPLES < sample_count / WAVEFORM_EMIT_SAMPLES {
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

            let _ = capture_client.ReleaseBuffer(num_frames);
        }
    }
}

// ─── Device Resolution ─────────────────────────────────────────────────────

/// Maps a DeviceSelection to a thread identity value.
/// AutoDetect (regardless of endpoint ID) always maps to None — all auto-detect
/// recordings share one capture thread. ByName maps to Some(name) — each named
/// device gets its own thread.
fn device_identity(device: &DeviceSelection) -> Option<String> {
    match device {
        DeviceSelection::AutoDetect(_) => None,
        DeviceSelection::ByName(name) => Some(name.clone()),
    }
}

/// Fallback device resolution: eConsole -> eCommunications.
/// Uses eConsole (Windows "Default device") to match the device monitor's tracking.
unsafe fn resolve_auto_fallback(
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
) -> Result<windows::Win32::Media::Audio::IMMDevice, AudioError> {
    use windows::Win32::Media::Audio::*;
    enumerator
        .GetDefaultAudioEndpoint(eCapture, eConsole)
        .or_else(|_| {
            log::warn!("[AudioRecorder] eConsole default not found, trying eCommunications");
            enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications)
        })
        .map_err(|_| AudioError::NoDevice)
}

/// Resolve an audio capture device from a DeviceSelection.
unsafe fn resolve_device(
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
    device: &DeviceSelection,
    pkey_friendly_name: &windows::Win32::Foundation::PROPERTYKEY,
) -> Result<windows::Win32::Media::Audio::IMMDevice, AudioError> {
    use windows::Win32::Media::Audio::*;
    use windows::core::PCWSTR;

    match device {
        DeviceSelection::AutoDetect(Some(id)) => {
            let wide_id: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            let pcwstr = PCWSTR(wide_id.as_ptr());
            match enumerator.GetDevice(pcwstr) {
                Ok(dev) => {
                    log::info!("[AudioRecorder] Resolved device by monitor endpoint ID");
                    Ok(dev)
                }
                Err(e) => {
                    log::warn!(
                        "[AudioRecorder] Monitor endpoint ID not found ({}), falling back to system default",
                        e
                    );
                    resolve_auto_fallback(enumerator)
                }
            }
        }
        DeviceSelection::AutoDetect(None) => {
            log::info!("[AudioRecorder] No monitor ID, using system default");
            resolve_auto_fallback(enumerator)
        }
        DeviceSelection::ByName(name) => {
            let collection = enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .map_err(|e| AudioError::DeviceError(format!("EnumAudioEndpoints: {}", e)))?;

            let count = collection
                .GetCount()
                .map_err(|e| AudioError::DeviceError(format!("GetCount: {}", e)))?;

            for i in 0..count {
                if let Ok(dev) = collection.Item(i) {
                    if let Ok(friendly) = get_device_friendly_name(&dev, pkey_friendly_name) {
                        if friendly == *name {
                            return Ok(dev);
                        }
                    }
                }
            }

            log::warn!(
                "[AudioRecorder] Configured microphone '{}' not found, falling back to system default",
                name
            );
            resolve_auto_fallback(enumerator)
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

// ─── Audio Processing (unchanged) ──────────────────────────────────────────

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

/// Resample from source_rate to target_rate.
/// Uses averaging for integer-ratio downsampling (e.g. 48kHz -> 16kHz),
/// linear interpolation as fallback for non-integer ratios.
fn resample(samples: &[i16], source_rate: u32, target_rate: u32) -> Vec<i16> {
    if source_rate == target_rate || samples.is_empty() {
        return samples.to_vec();
    }

    // Integer-ratio downsampling with averaging (e.g. 48000/16000 = 3)
    if source_rate > target_rate && source_rate % target_rate == 0 {
        let ratio = (source_rate / target_rate) as usize;
        return samples
            .chunks(ratio)
            .map(|chunk| {
                let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
                (sum / chunk.len() as i32) as i16
            })
            .collect();
    }

    // Fallback: linear interpolation for non-integer ratios
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

/// Window size for speech detection (~50ms at 16kHz)
const SPEECH_WINDOW_SAMPLES: usize = 800;

fn has_speech(samples: &[i16]) -> (bool, f64) {
    if samples.is_empty() {
        return (false, 0.0);
    }

    let mut max_rms: f64 = 0.0;

    for window in samples.chunks(SPEECH_WINDOW_SAMPLES) {
        let sum_sq: f64 = window.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum_sq / window.len() as f64).sqrt();
        if rms > max_rms {
            max_rms = rms;
        }
    }

    (max_rms >= SILENCE_RMS_THRESHOLD, max_rms)
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

// ─── Device Listing ────────────────────────────────────────────────────────

pub fn list_microphones() -> Result<Vec<MicrophoneInfo>, AudioError> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    let pkey_friendly_name = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    let pkey_form_factor = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e),
        pid: 0,
    };

    unsafe {
        // COM may already be initialized on this thread (STA by WebView2).
        // RPC_E_CHANGED_MODE means COM is usable — only uninit if we initialized it.
        let com_hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _com_guard = if com_hr.is_ok() {
            Some(ComUninitGuard)
        } else {
            None
        };

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| AudioError::DeviceError(format!("CoCreateInstance failed: {}", e)))?;

        // Get default console device ID for accurate is_default comparison
        let default_id = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok()
            .and_then(|dev| dev.GetId().ok())
            .map(|pwstr| {
                let s = pwstr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
                s
            })
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
            let device = match collection.Item(i) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let name = match get_device_friendly_name(&device, &pkey_friendly_name) {
                Ok(n) if !n.is_empty() => n,
                _ => continue,
            };

            // Get endpoint ID
            let id = device
                .GetId()
                .ok()
                .map(|pwstr| {
                    let s = pwstr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
                    s
                })
                .unwrap_or_default();

            // Get form factor
            let form_factor = get_form_factor(&device, &pkey_form_factor);

            mics.push(MicrophoneInfo {
                is_default: !id.is_empty() && id == default_id,
                name,
                id,
                form_factor,
            });
        }

        Ok(mics)
    }
}

/// Extract the form factor string from device properties.
/// Returns "Bluetooth", "USB", "Line", "Microphone", "Headset", or "Audio".
unsafe fn get_form_factor(
    device: &windows::Win32::Media::Audio::IMMDevice,
    pkey: &windows::Win32::Foundation::PROPERTYKEY,
) -> String {
    use windows::Win32::System::Com::{CoTaskMemFree, STGM_READ};

    // First check if Bluetooth or USB via endpoint ID string
    let device_id = device
        .GetId()
        .ok()
        .map(|pwstr| {
            let s = pwstr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(pwstr.as_ptr() as *const std::ffi::c_void));
            s
        })
        .unwrap_or_default();

    let id_upper = device_id.to_uppercase();
    if id_upper.contains("BTHENUM") || id_upper.contains("BTH") {
        return "Bluetooth".to_string();
    }

    // Fall back to PKEY_AudioEndpoint_FormFactor property
    let store = match device.OpenPropertyStore(STGM_READ) {
        Ok(s) => s,
        Err(_) => return "Audio".to_string(),
    };

    let prop = match store.GetValue(pkey) {
        Ok(p) => p,
        Err(_) => return "Audio".to_string(),
    };

    // Form factor is stored as VT_UI4 (u32)
    let form_factor_u32 = prop.Anonymous.Anonymous.Anonymous.uintVal;

    match form_factor_u32 {
        2 => "Line".to_string(),
        4 => "Microphone".to_string(),
        5 => "Headset".to_string(),
        _ => {
            if id_upper.contains("USB") {
                "USB".to_string()
            } else {
                "Audio".to_string()
            }
        }
    }
}

// ─── Error Types ───────────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::audio::device_monitor::DeviceSelection;

    #[test]
    fn test_device_identity_auto_detect_none() {
        let sel = DeviceSelection::AutoDetect(None);
        assert_eq!(device_identity(&sel), None);
    }

    #[test]
    fn test_device_identity_auto_detect_with_id() {
        let sel = DeviceSelection::AutoDetect(Some("endpoint-123".to_string()));
        assert_eq!(device_identity(&sel), None);
    }

    #[test]
    fn test_device_identity_by_name() {
        let sel = DeviceSelection::ByName("My USB Mic".to_string());
        assert_eq!(device_identity(&sel), Some("My USB Mic".to_string()));
    }

    #[test]
    fn test_device_identity_by_name_empty() {
        let sel = DeviceSelection::ByName("".to_string());
        assert_eq!(device_identity(&sel), Some("".to_string()));
    }
}

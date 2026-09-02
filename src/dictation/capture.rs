use crate::dictation::types::{
    CapturedAudioChunk, DictationCaptureConfig, DictationCaptureEvent, DictationDeviceId,
    RawAudioChunk,
};
use crate::dictation::visualizer::AudioVisualiser;
use anyhow::{bail, Context, Result};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(target_os = "macos")]
use objc::declare::ClassDecl;
#[cfg(target_os = "macos")]
use objc::runtime::{Class, Object, Sel, BOOL, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::{c_void, CStr};
#[cfg(target_os = "macos")]
use std::sync::Once;

const I16_NORMALIZATION_FACTOR: f32 = 32_768.0;

pub(crate) fn mix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channel_count = usize::from(channels.max(1));

    if channel_count == 1 {
        return samples
            .iter()
            .map(|sample| sample.clamp(-1.0, 1.0))
            .collect();
    }

    let mut mono = Vec::with_capacity(samples.len().div_ceil(channel_count));
    for frame in samples.chunks(channel_count) {
        if frame.is_empty() {
            continue;
        }

        let sum = frame
            .iter()
            .fold(0.0_f32, |acc, sample| acc + sample.clamp(-1.0, 1.0));
        mono.push((sum / frame.len() as f32).clamp(-1.0, 1.0));
    }

    mono
}

pub(crate) fn resample_linear(
    samples: &[f32],
    input_sample_rate_hz: u32,
    output_sample_rate_hz: u32,
) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let input_rate = input_sample_rate_hz.max(1);
    let output_rate = output_sample_rate_hz.max(1);

    if input_rate == output_rate || samples.len() == 1 {
        return samples.to_vec();
    }

    let output_len = ((samples.len() as f64 * output_rate as f64) / input_rate as f64)
        .round()
        .max(1.0) as usize;

    if output_len == 1 {
        return vec![samples[0].clamp(-1.0, 1.0)];
    }

    let mut output = Vec::with_capacity(output_len);
    let input_span = (samples.len() - 1) as f64;
    let output_span = (output_len - 1) as f64;

    for index in 0..output_len {
        let position = (index as f64 * input_span) / output_span;
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        let fraction = (position - lower as f64) as f32;
        let lower_sample = samples[lower].clamp(-1.0, 1.0);
        let upper_sample = samples[upper].clamp(-1.0, 1.0);
        output.push((lower_sample + (upper_sample - lower_sample) * fraction).clamp(-1.0, 1.0));
    }

    output
}

pub(crate) fn normalize_chunk(
    raw_chunk: RawAudioChunk,
    config: &DictationCaptureConfig,
) -> CapturedAudioChunk {
    let mono = mix_to_mono(&raw_chunk.samples, raw_chunk.channels);
    let sample_rate_hz = config.sample_rate_hz.max(1);
    let samples = resample_linear(&mono, raw_chunk.sample_rate_hz.max(1), sample_rate_hz)
        .into_iter()
        .map(|sample| sample.clamp(-1.0, 1.0))
        .collect::<Vec<_>>();
    let duration = duration_from_samples(samples.len(), sample_rate_hz);

    CapturedAudioChunk {
        sample_rate_hz,
        samples,
        duration,
    }
}

pub(crate) fn run_processor(
    raw_rx: mpsc::Receiver<RawAudioChunk>,
    event_tx: async_channel::Sender<DictationCaptureEvent>,
    config: DictationCaptureConfig,
) {
    let sample_rate_hz = config.sample_rate_hz.max(1);
    let chunk_sample_count = samples_for_duration(sample_rate_hz, config.chunk_duration).max(1);
    let mut visualiser = AudioVisualiser::new_speech(sample_rate_hz);
    let mut pending_samples: Vec<f32> = Vec::with_capacity(chunk_sample_count);

    while let Ok(raw_chunk) = raw_rx.recv() {
        let normalized = normalize_chunk(raw_chunk, &config);

        // Feed samples to the FFT visualizer; emit bar levels when a full
        // window has been processed.
        if let Some(bars) = visualiser.feed(&normalized.samples) {
            if event_tx
                .try_send(DictationCaptureEvent::Bars(bars))
                .is_err()
            {
                tracing::debug!(
                    category = "DICTATION",
                    "Dropping dictation waveform bars because the UI event channel is full"
                );
            }
        }

        pending_samples.extend_from_slice(&normalized.samples);

        while pending_samples.len() >= chunk_sample_count {
            let tail = pending_samples.split_off(chunk_sample_count);
            let chunk_samples = std::mem::replace(&mut pending_samples, tail);
            let sample_count = chunk_samples.len();
            let duration = duration_from_samples(chunk_samples.len(), sample_rate_hz);
            if event_tx
                .try_send(DictationCaptureEvent::Chunk(CapturedAudioChunk {
                    sample_rate_hz,
                    samples: chunk_samples,
                    duration,
                }))
                .is_err()
            {
                tracing::warn!(
                    category = "DICTATION",
                    sample_count,
                    "Dropping dictation audio chunk because the UI event channel is full"
                );
            }
        }
    }

    // Flush remaining samples as a tail chunk. This is the last audio the
    // user spoke before stop — send it blocking so a momentarily full event
    // channel cannot silently drop the end of the recording. The stop
    // coordinator is draining the receiver at this point, and if the session
    // was abandoned instead the receiver is dropped, which makes
    // send_blocking return an error immediately rather than hang.
    if !pending_samples.is_empty() {
        let duration = duration_from_samples(pending_samples.len(), sample_rate_hz);
        if event_tx
            .send_blocking(DictationCaptureEvent::Chunk(CapturedAudioChunk {
                sample_rate_hz,
                samples: pending_samples,
                duration,
            }))
            .is_err()
        {
            tracing::warn!(
                category = "DICTATION",
                "Dictation event channel closed before the tail chunk could be flushed"
            );
        }
    }

    let _ = event_tx.send_blocking(DictationCaptureEvent::EndOfStream);
}

fn samples_for_duration(sample_rate_hz: u32, duration: Duration) -> usize {
    (duration.as_secs_f64() * sample_rate_hz as f64)
        .round()
        .max(1.0) as usize
}

fn duration_from_samples(sample_count: usize, sample_rate_hz: u32) -> Duration {
    if sample_count == 0 {
        return Duration::ZERO;
    }

    Duration::from_secs_f64(sample_count as f64 / sample_rate_hz.max(1) as f64)
}

pub struct DictationCaptureHandle {
    #[cfg(target_os = "macos")]
    session: *mut Object,
    #[cfg(target_os = "macos")]
    delegate: *mut Object,
    #[cfg(target_os = "macos")]
    queue: *mut c_void,
    #[cfg(target_os = "macos")]
    sender_ptr: *mut c_void,
    processor_thread: Option<JoinHandle<()>>,
}

// SAFETY: The handle owns thread-safe AVFoundation resources and performs
// synchronized shutdown before freeing the callback sender.
unsafe impl Send for DictationCaptureHandle {}

#[cfg(target_os = "macos")]
impl Drop for DictationCaptureHandle {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: `self.session` is a valid owned AVCaptureSession.
            let _: () = msg_send![self.session, stopRunning];

            extern "C" fn noop(_ctx: *mut c_void) {}

            // SAFETY: Draining the queue ensures in-flight callbacks complete before sender
            // teardown and Objective-C object release.
            dispatch_sync_f(self.queue, std::ptr::null_mut(), noop);

            // SAFETY: `_sender` was installed by start_capture on this delegate instance.
            (*self.delegate).set_ivar::<*mut c_void>("_sender", std::ptr::null_mut());

            if !self.sender_ptr.is_null() {
                // SAFETY: `sender_ptr` originated from Box::into_raw in start_capture.
                let _ = Box::from_raw(self.sender_ptr as *mut mpsc::SyncSender<RawAudioChunk>);
            }

            // SAFETY: These are owned Objective-C references acquired with alloc/init.
            let _: () = msg_send![self.delegate, release];
            let _: () = msg_send![self.session, release];

            // SAFETY: The queue was created with dispatch_queue_create.
            dispatch_release(self.queue);
        }

        if let Some(processor_thread) = self.processor_thread.take() {
            tracing::debug!(
                category = "DICTATION",
                "Detaching dictation processor thread after capture teardown"
            );
            drop(processor_thread);
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl Drop for DictationCaptureHandle {
    fn drop(&mut self) {
        if let Some(processor_thread) = self.processor_thread.take() {
            drop(processor_thread);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn start_capture(
    config: DictationCaptureConfig,
    device_id: Option<&DictationDeviceId>,
) -> Result<(
    async_channel::Receiver<DictationCaptureEvent>,
    DictationCaptureHandle,
)> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Device)?;
    static REGISTER: Once = Once::new();
    REGISTER.call_once(register_delegate_class);

    // ~41 events/s at steady state (40 ms chunks + 60 ms bar windows), so
    // 4096 gives the UI drain loop ~100 s of slack before mid-recording
    // chunks would be dropped — a stalled pump must not cost the user audio.
    let (event_tx, event_rx) = async_channel::bounded(4096);
    let (raw_tx, raw_rx) = mpsc::sync_channel::<RawAudioChunk>(128);
    let processor_thread = std::thread::Builder::new()
        .name("dictation-processor".to_string())
        .spawn(move || run_processor(raw_rx, event_tx, config))
        .context("failed to spawn dictation processor thread")?;

    unsafe {
        // SAFETY: alloc/init returns an owned AVCaptureSession reference.
        let session: *mut Object = msg_send![class!(AVCaptureSession), alloc];
        let session: *mut Object = msg_send![session, init];
        if session.is_null() {
            return Err(anyhow::anyhow!("AVCaptureSession alloc/init returned null"));
        }

        let device = match resolve_device(device_id).context("failed to resolve input device") {
            Ok(device) => device,
            Err(error) => {
                cleanup_start_capture_resources(
                    session,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                return Err(error);
            }
        };

        let mut error: *mut Object = std::ptr::null_mut();
        // SAFETY: Objective-C convenience constructor returns a borrowed/autoreleased input.
        let input: *mut Object = msg_send![
            class!(AVCaptureDeviceInput),
            deviceInputWithDevice: device
            error: &mut error
        ];
        if input.is_null() {
            let error_summary = nserror_to_string(error);
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(anyhow::anyhow!(
                "failed to create AVCaptureDeviceInput: {error_summary}"
            ));
        }

        let can_add_input: BOOL = msg_send![session, canAddInput: input];
        if can_add_input != YES {
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            bail!("AVCaptureSession rejected audio input");
        }
        let _: () = msg_send![session, addInput: input];

        // SAFETY: alloc/init returns an owned AVCaptureAudioDataOutput reference.
        let output: *mut Object = msg_send![class!(AVCaptureAudioDataOutput), alloc];
        let output: *mut Object = msg_send![output, init];
        if output.is_null() {
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            bail!("AVCaptureAudioDataOutput alloc/init returned null");
        }

        // SAFETY: GCD queue label is a static NUL-terminated string.
        let queue: *mut c_void = dispatch_queue_create(
            c"com.scriptkit.dictation.capture".as_ptr(),
            std::ptr::null_mut(),
        );
        if queue.is_null() {
            cleanup_start_capture_resources(
                session,
                output,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            bail!("dispatch_queue_create returned null for dictation capture queue");
        }

        let delegate_class = match Class::get("SKDictationAudioDelegate") {
            Some(delegate_class) => delegate_class,
            None => {
                cleanup_start_capture_resources(
                    session,
                    output,
                    std::ptr::null_mut(),
                    queue,
                    std::ptr::null_mut(),
                );
                bail!("SKDictationAudioDelegate class was not registered");
            }
        };

        // SAFETY: alloc/init returns an owned delegate object.
        let delegate: *mut Object = msg_send![delegate_class, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        if delegate.is_null() {
            cleanup_start_capture_resources(
                session,
                output,
                std::ptr::null_mut(),
                queue,
                std::ptr::null_mut(),
            );
            bail!("SKDictationAudioDelegate alloc/init returned null");
        }

        let sender_ptr = Box::into_raw(Box::new(raw_tx)) as *mut c_void;
        // SAFETY: The `_sender` ivar exists on the registered delegate class.
        (*delegate).set_ivar::<*mut c_void>("_sender", sender_ptr);

        let _: () =
            msg_send![output, setSampleBufferDelegate: delegate queue: queue as *mut Object];

        let can_add_output: BOOL = msg_send![session, canAddOutput: output];
        if can_add_output != YES {
            cleanup_start_capture_resources(session, output, delegate, queue, sender_ptr);
            bail!("AVCaptureSession rejected audio output");
        }

        let _: () = msg_send![session, addOutput: output];
        let _: () = msg_send![output, release];
        let _: () = msg_send![session, startRunning];

        Ok((
            event_rx,
            DictationCaptureHandle {
                session,
                delegate,
                queue,
                sender_ptr,
                processor_thread: Some(processor_thread),
            },
        ))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_capture(
    _config: DictationCaptureConfig,
    _device_id: Option<&DictationDeviceId>,
) -> Result<(
    async_channel::Receiver<DictationCaptureEvent>,
    DictationCaptureHandle,
)> {
    bail!("dictation capture is only supported on macOS")
}

#[cfg(target_os = "macos")]
fn register_delegate_class() {
    let superclass = class!(NSObject);
    let Some(mut decl) = ClassDecl::new("SKDictationAudioDelegate", superclass) else {
        return;
    };

    decl.add_ivar::<*mut c_void>("_sender");

    unsafe {
        // SAFETY: The method signature matches AVCaptureAudioDataOutputSampleBufferDelegate.
        decl.add_method(
            sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
            capture_callback
                as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
        );
    }

    decl.register();
}

#[cfg(target_os = "macos")]
extern "C" fn capture_callback(
    this: &mut Object,
    _sel: Sel,
    _output: *mut Object,
    sample_buffer: *mut Object,
    _connection: *mut Object,
) {
    unsafe {
        // SAFETY: The callback only extracts raw PCM and enqueues it to a bounded channel.
        let Some(raw_chunk) = extract_pcm_chunk(sample_buffer) else {
            return;
        };

        let sender_ptr = *this.get_ivar::<*mut c_void>("_sender");
        if sender_ptr.is_null() {
            return;
        }

        let sender = &*(sender_ptr as *const mpsc::SyncSender<RawAudioChunk>);
        let _ = sender.try_send(raw_chunk);
    }
}

#[cfg(target_os = "macos")]
unsafe fn resolve_device(device_id: Option<&DictationDeviceId>) -> Result<*mut Object> {
    if let Some(device_id) = device_id {
        // SAFETY: Objective-C messaging is used with AVCaptureDevice enumeration selectors.
        let devices: *mut Object =
            msg_send![class!(AVCaptureDevice), devicesWithMediaType: av_media_type_audio()];
        if devices.is_null() {
            bail!("AVCaptureDevice devicesWithMediaType returned null");
        }

        let count: usize = msg_send![devices, count];
        for index in 0..count {
            let device: *mut Object = msg_send![devices, objectAtIndex: index];
            if device.is_null() {
                continue;
            }

            let unique_id_obj: *mut Object = msg_send![device, uniqueID];
            if nsstring_to_string(unique_id_obj).as_deref() == Some(device_id.0.as_str()) {
                return Ok(device);
            }
        }

        bail!("no audio input device found for id {}", device_id.0);
    }

    // SAFETY: defaultDeviceWithMediaType returns a borrowed device reference.
    let device: *mut Object =
        msg_send![class!(AVCaptureDevice), defaultDeviceWithMediaType: av_media_type_audio()];
    if device.is_null() {
        bail!("no default audio input device available");
    }

    Ok(device)
}

#[cfg(target_os = "macos")]
unsafe fn cleanup_start_capture_resources(
    session: *mut Object,
    output: *mut Object,
    delegate: *mut Object,
    queue: *mut c_void,
    sender_ptr: *mut c_void,
) {
    if !sender_ptr.is_null() {
        // SAFETY: `sender_ptr` originates from Box::into_raw in start_capture.
        let _ = Box::from_raw(sender_ptr as *mut mpsc::SyncSender<RawAudioChunk>);
    }

    if !delegate.is_null() {
        // SAFETY: Owned Objective-C object created with alloc/init in start_capture.
        let _: () = msg_send![delegate, release];
    }

    if !output.is_null() {
        // SAFETY: Owned Objective-C object created with alloc/init in start_capture.
        let _: () = msg_send![output, release];
    }

    if !session.is_null() {
        // SAFETY: Owned Objective-C object created with alloc/init in start_capture.
        let _: () = msg_send![session, release];
    }

    if !queue.is_null() {
        // SAFETY: Queue was created with dispatch_queue_create.
        dispatch_release(queue);
    }
}

#[cfg(target_os = "macos")]
unsafe fn nserror_to_string(error: *mut Object) -> String {
    if error.is_null() {
        return "unknown NSError".to_string();
    }

    // SAFETY: `error` is an NSError-like object provided by AVFoundation.
    let domain_obj: *mut Object = msg_send![error, domain];
    let code: i64 = msg_send![error, code];
    let description_obj: *mut Object = msg_send![error, localizedDescription];
    let domain = nsstring_to_string(domain_obj).unwrap_or_else(|| "unknown-domain".to_string());
    let description =
        nsstring_to_string(description_obj).unwrap_or_else(|| "unknown-description".to_string());
    format!("domain={domain}, code={code}, description={description}")
}

#[cfg(target_os = "macos")]
fn av_media_type_audio() -> *mut Object {
    unsafe {
        // SAFETY: The UTF-8 string literal is NUL-terminated and valid for NSString construction.
        let media_type: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c"soun".as_ptr()];
        media_type
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }

    // SAFETY: `value` is NSString-compatible and UTF8String returns a borrowed C string.
    let utf8: *const i8 = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }

    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
pub(crate) unsafe fn extract_pcm_chunk(sample_buffer: *mut Object) -> Option<RawAudioChunk> {
    let format_description = CMSampleBufferGetFormatDescription(sample_buffer as CMSampleBufferRef);
    if format_description.is_null() {
        return None;
    }

    let stream_description = CMAudioFormatDescriptionGetStreamBasicDescription(format_description);
    if stream_description.is_null() {
        return None;
    }

    let stream_description = &*stream_description;
    if stream_description.m_format_id != K_AUDIO_FORMAT_LINEAR_PCM
        || stream_description.m_channels_per_frame == 0
        || stream_description.m_sample_rate <= 0.0
    {
        return None;
    }

    let format_flags = stream_description.m_format_flags;
    if (format_flags & K_LINEAR_PCM_FORMAT_FLAG_IS_BIG_ENDIAN) != 0 {
        return None;
    }

    let is_float32 = stream_description.m_bits_per_channel == 32
        && (format_flags & K_LINEAR_PCM_FORMAT_FLAG_IS_FLOAT) != 0;
    let is_int16 = stream_description.m_bits_per_channel == 16
        && (format_flags & K_LINEAR_PCM_FORMAT_FLAG_IS_SIGNED_INTEGER) != 0;
    if !is_float32 && !is_int16 {
        return None;
    }

    let frame_count_i64 = CMSampleBufferGetNumSamples(sample_buffer as CMSampleBufferRef);
    if frame_count_i64 <= 0 {
        return None;
    }
    let frame_count = usize::try_from(frame_count_i64).ok()?;
    let channel_count = usize::try_from(stream_description.m_channels_per_frame).ok()?;

    let mut required_size = 0usize;
    let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
        sample_buffer as CMSampleBufferRef,
        &mut required_size,
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        K_CMSAMPLE_BUFFER_FLAG_AUDIO_BUFFER_LIST_ASSURE_16_BYTE_ALIGNMENT,
        std::ptr::null_mut(),
    );
    if status != 0 || required_size == 0 {
        return None;
    }

    let mut buffer_storage = vec![0_u8; required_size];
    let buffer_list = buffer_storage.as_mut_ptr() as *mut AudioBufferList;
    let mut block_buffer: CMBlockBufferRef = std::ptr::null_mut();
    let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
        sample_buffer as CMSampleBufferRef,
        std::ptr::null_mut(),
        buffer_list,
        required_size,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        K_CMSAMPLE_BUFFER_FLAG_AUDIO_BUFFER_LIST_ASSURE_16_BYTE_ALIGNMENT,
        &mut block_buffer,
    );
    if status != 0 {
        return None;
    }

    let _block_buffer_guard = BlockBufferGuard(block_buffer);
    let buffer_count = usize::try_from((*buffer_list).m_number_buffers).ok()?;
    let is_non_interleaved = (format_flags & K_LINEAR_PCM_FORMAT_FLAG_IS_NON_INTERLEAVED) != 0;

    let samples = if is_non_interleaved {
        extract_non_interleaved_samples(
            buffer_list,
            buffer_count,
            frame_count,
            channel_count,
            is_float32,
        )?
    } else {
        extract_interleaved_samples(buffer_list, frame_count, channel_count, is_float32)?
    };

    Some(RawAudioChunk {
        sample_rate_hz: stream_description
            .m_sample_rate
            .round()
            .clamp(1.0, u32::MAX as f64) as u32,
        channels: u16::try_from(channel_count).ok()?,
        samples,
    })
}

#[cfg(target_os = "macos")]
unsafe fn extract_interleaved_samples(
    buffer_list: *const AudioBufferList,
    frame_count: usize,
    channel_count: usize,
    is_float32: bool,
) -> Option<Vec<f32>> {
    let buffer = audio_buffer_at(buffer_list, 0)?;
    let expected_sample_count = frame_count.checked_mul(channel_count)?;
    let bytes_per_sample = if is_float32 { 4_usize } else { 2_usize };
    let available_sample_count = usize::try_from(buffer.m_data_byte_size).ok()? / bytes_per_sample;
    if available_sample_count < expected_sample_count || buffer.m_data.is_null() {
        return None;
    }

    if is_float32 {
        let slice = std::slice::from_raw_parts(buffer.m_data as *const f32, expected_sample_count);
        Some(slice.iter().map(|sample| sample.clamp(-1.0, 1.0)).collect())
    } else {
        let slice = std::slice::from_raw_parts(buffer.m_data as *const i16, expected_sample_count);
        Some(
            slice
                .iter()
                .map(|sample| (*sample as f32 / I16_NORMALIZATION_FACTOR).clamp(-1.0, 1.0))
                .collect(),
        )
    }
}

#[cfg(target_os = "macos")]
unsafe fn extract_non_interleaved_samples(
    buffer_list: *const AudioBufferList,
    buffer_count: usize,
    frame_count: usize,
    channel_count: usize,
    is_float32: bool,
) -> Option<Vec<f32>> {
    if buffer_count < channel_count {
        return None;
    }

    let bytes_per_sample = if is_float32 { 4_usize } else { 2_usize };
    let mut channel_slices = Vec::with_capacity(channel_count);

    for channel_index in 0..channel_count {
        let buffer = audio_buffer_at(buffer_list, channel_index)?;
        if buffer.m_data.is_null() {
            return None;
        }

        let available_sample_count =
            usize::try_from(buffer.m_data_byte_size).ok()? / bytes_per_sample;
        if available_sample_count < frame_count {
            return None;
        }

        if is_float32 {
            let slice = std::slice::from_raw_parts(buffer.m_data as *const f32, frame_count);
            channel_slices.push(ChannelData::Float(slice));
        } else {
            let slice = std::slice::from_raw_parts(buffer.m_data as *const i16, frame_count);
            channel_slices.push(ChannelData::Int16(slice));
        }
    }

    let mut samples = Vec::with_capacity(frame_count.checked_mul(channel_count)?);
    for frame_index in 0..frame_count {
        for channel in &channel_slices {
            let sample = match channel {
                ChannelData::Float(values) => values[frame_index],
                ChannelData::Int16(values) => values[frame_index] as f32 / I16_NORMALIZATION_FACTOR,
            };
            samples.push(sample.clamp(-1.0, 1.0));
        }
    }

    Some(samples)
}

#[cfg(target_os = "macos")]
unsafe fn audio_buffer_at(
    buffer_list: *const AudioBufferList,
    index: usize,
) -> Option<&'static AudioBuffer> {
    let buffer_count = usize::try_from((*buffer_list).m_number_buffers).ok()?;
    if index >= buffer_count {
        return None;
    }

    let base = std::ptr::addr_of!((*buffer_list).m_buffers) as *const AudioBuffer;
    Some(&*base.add(index))
}

#[cfg(target_os = "macos")]
struct BlockBufferGuard(CMBlockBufferRef);

#[cfg(target_os = "macos")]
impl Drop for BlockBufferGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                // SAFETY: The block buffer was retained by
                // CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer.
                CFRelease(self.0.cast());
            }
        }
    }
}

#[cfg(target_os = "macos")]
enum ChannelData<'a> {
    Float(&'a [f32]),
    Int16(&'a [i16]),
}

#[cfg(target_os = "macos")]
const K_AUDIO_FORMAT_LINEAR_PCM: u32 = 0x6c70636d;
#[cfg(target_os = "macos")]
const K_LINEAR_PCM_FORMAT_FLAG_IS_FLOAT: u32 = 1 << 0;
#[cfg(target_os = "macos")]
const K_LINEAR_PCM_FORMAT_FLAG_IS_BIG_ENDIAN: u32 = 1 << 1;
#[cfg(target_os = "macos")]
const K_LINEAR_PCM_FORMAT_FLAG_IS_SIGNED_INTEGER: u32 = 1 << 2;
#[cfg(target_os = "macos")]
const K_LINEAR_PCM_FORMAT_FLAG_IS_NON_INTERLEAVED: u32 = 1 << 5;
#[cfg(target_os = "macos")]
const K_CMSAMPLE_BUFFER_FLAG_AUDIO_BUFFER_LIST_ASSURE_16_BYTE_ALIGNMENT: u32 = 1 << 0;

#[cfg(target_os = "macos")]
type CMSampleBufferRef = *mut c_void;
#[cfg(target_os = "macos")]
type CMFormatDescriptionRef = *const c_void;
#[cfg(target_os = "macos")]
type CMBlockBufferRef = *mut c_void;

#[cfg(target_os = "macos")]
#[repr(C)]
struct AudioStreamBasicDescription {
    m_sample_rate: f64,
    m_format_id: u32,
    m_format_flags: u32,
    m_bytes_per_packet: u32,
    m_frames_per_packet: u32,
    m_bytes_per_frame: u32,
    m_channels_per_frame: u32,
    m_bits_per_channel: u32,
    m_reserved: u32,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct AudioBuffer {
    m_number_channels: u32,
    m_data_byte_size: u32,
    m_data: *mut c_void,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct AudioBufferList {
    m_number_buffers: u32,
    m_buffers: [AudioBuffer; 1],
}

#[cfg(target_os = "macos")]
extern "C" {
    fn CMSampleBufferGetFormatDescription(sbuf: CMSampleBufferRef) -> CMFormatDescriptionRef;
    fn CMSampleBufferGetNumSamples(sbuf: CMSampleBufferRef) -> isize;
    fn CMAudioFormatDescriptionGetStreamBasicDescription(
        desc: CMFormatDescriptionRef,
    ) -> *const AudioStreamBasicDescription;
    fn CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
        sbuf: CMSampleBufferRef,
        buffer_list_size_needed_out: *mut usize,
        buffer_list_out: *mut AudioBufferList,
        buffer_list_size: usize,
        block_buffer_structure_allocator: *mut c_void,
        block_buffer_block_allocator: *mut c_void,
        flags: u32,
        block_buffer_out: *mut CMBlockBufferRef,
    ) -> i32;
    fn dispatch_queue_create(label: *const i8, attr: *mut c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
    fn dispatch_sync_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn CFRelease(value: *const c_void);
}

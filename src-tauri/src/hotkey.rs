use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
};
use tauri::AppHandle;
use tokio::sync::mpsc;

use crate::events::{self, event_names};
use crate::recording::RecordingCommand;
use crate::settings::ActivationMode;

// `CGEventTapEnable` is not re-exported by the `core-graphics` crate, so we
// declare it here. We need to call it from inside the tap callback to recover
// when macOS disables the tap (kCGEventTapDisabledByTimeout or
// kCGEventTapDisabledByUserInput — common after a callback overrun, secure
// input, or a stale TCC entry on rebuild).
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

/// Thread-safe wrapper around a raw CFMachPortRef so the tap's mach port can
/// be handed into the event-tap callback for re-enable. The underlying
/// CoreFoundation object is reference-counted and thread-safe for the read
/// operations we perform (CGEventTapEnable).
#[derive(Clone, Copy)]
struct MachPortHandle(CFMachPortRef);
unsafe impl Send for MachPortHandle {}
unsafe impl Sync for MachPortHandle {}

/// Handle to a running Fn key monitor. Calling `stop()` terminates the
/// CFRunLoop, causing the monitor thread to exit cleanly.
pub struct FnKeyMonitorHandle {
    run_loop: Arc<Mutex<Option<CFRunLoop>>>,
    /// `true` while the CGEventTap thread is running. Note this is a
    /// thread-liveness flag, not a tap-health flag — the tap itself can be
    /// disabled by macOS while this is still `true`. Use `is_active()` for
    /// an accurate "is Fn actually working?" answer.
    active: Arc<AtomicBool>,
    /// Set by the tap callback when macOS disabled the tap and the callback
    /// could not re-enable it (e.g. the mach port wasn't stored yet). The
    /// watchdog in `lib.rs` polls this and triggers a full restart.
    needs_restart: Arc<AtomicBool>,
}

impl FnKeyMonitorHandle {
    /// Stop the monitor's run loop. Idempotent — safe to call multiple times.
    pub fn stop(&self) {
        if let Some(run_loop) = self.run_loop.lock().take() {
            run_loop.stop();
        }
    }

    /// Returns `true` if the monitor thread is running AND the tap is
    /// believed healthy. Returns `false` if macOS has disabled the tap and
    /// the callback flagged a restart (meaning events aren't flowing).
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst) && !self.needs_restart.load(Ordering::SeqCst)
    }

    /// Returns `true` if the tap callback has flagged that a full restart is
    /// needed. The watchdog in `lib.rs` uses this to trigger
    /// `restart_fn_key_monitor_inner`.
    pub fn needs_restart(&self) -> bool {
        self.needs_restart.load(Ordering::SeqCst)
    }
}

/// Start monitoring for Fn key events using a CGEventTap.
/// This runs on a dedicated thread and sends commands directly to the
/// recording channel. Returns a handle that can stop the monitor and a
/// boolean indicating whether the CGEventTap was successfully created.
///
/// The `app` handle is only used to emit a permissions-status event if the
/// CGEventTap fails to create (accessibility permission not granted).
pub fn start_fn_key_monitor(
    app: AppHandle,
    tx: mpsc::UnboundedSender<RecordingCommand>,
    activation_mode: ActivationMode,
) -> (FnKeyMonitorHandle, bool) {
    let run_loop_slot: Arc<Mutex<Option<CFRunLoop>>> = Arc::new(Mutex::new(None));
    let run_loop_slot_clone = run_loop_slot.clone();
    let active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let active_clone = active.clone();
    let needs_restart: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let needs_restart_clone = needs_restart.clone();

    // Preflight Input Monitoring check. CGEventTap::new() will succeed
    // without this permission, but the tap silently delivers no events
    // from other apps — which looks exactly like "nothing happens when
    // I press Fn". Emitting early here lets the frontend show the
    // "Open Input Monitoring Settings" banner instead of leaving the
    // user mystified.
    if !crate::permissions::is_input_monitoring_trusted() {
        log::error!(
            "Input Monitoring permission NOT granted — CGEventTap will not \
             receive system-wide keyboard events. Prompting user."
        );
        // Trigger the TCC prompt if this is the first run. If access was
        // previously denied this is a no-op; the banner directs the user
        // to System Settings.
        let _ = crate::permissions::request_input_monitoring_access();
        crate::events::emit_event(
            &app,
            crate::events::event_names::PERMISSIONS_STATUS,
            crate::events::PermissionsPayload {
                microphone: crate::permissions::microphone_authorization_status()
                    == crate::permissions::MicrophoneAuthStatus::Authorized,
                accessibility: crate::permissions::is_accessibility_trusted(),
                input_monitoring: false,
            },
        );
        return (
            FnKeyMonitorHandle {
                run_loop: run_loop_slot,
                active,
                needs_restart,
            },
            false,
        );
    }

    // Slot holding the tap's mach port once the tap is created. The callback
    // reads from this to re-enable the tap when macOS disables it. The port
    // is stored inside the same thread that owns the tap, so the CFMachPort
    // outlives any callback invocation.
    let port_slot: Arc<Mutex<Option<MachPortHandle>>> = Arc::new(Mutex::new(None));
    let port_slot_cb = port_slot.clone();

    // The thread signals whether the CGEventTap was created successfully.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<bool>();

    std::thread::Builder::new()
        .name("fn-key-monitor".into())
        .spawn(move || {
            log::info!("Starting Fn key monitor (mode: {:?})", activation_mode);

            let fn_was_pressed = Arc::new(AtomicBool::new(false));
            let last_tap_time: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
            let is_recording = Arc::new(AtomicBool::new(false));

            // The secondary Fn flag bit
            const SECONDARY_FN_FLAG: u64 = 0x800000; // kCGEventFlagMaskSecondaryFn

            let tx_clone = tx.clone();
            let mode = activation_mode.clone();
            let fn_was_pressed_clone = fn_was_pressed.clone();
            let last_tap_time_clone = last_tap_time.clone();
            let is_recording_clone = is_recording.clone();
            let needs_restart_cb = needs_restart_clone.clone();

            let tap = CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                vec![
                    CGEventType::FlagsChanged,
                    CGEventType::TapDisabledByTimeout,
                    CGEventType::TapDisabledByUserInput,
                ],
                move |_proxy, event_type, event| {
                    // SAFETY: This closure is invoked through an `unsafe extern "C"` trampoline
                    // by Core Graphics. A panic here would unwind across the C ABI boundary,
                    // causing an immediate abort. We must catch any panic to prevent that.
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        // macOS disables taps in various scenarios (callback overrun,
                        // secure input, stale TCC handshake, etc.). When that happens
                        // we get one of these event types and the tap silently stops
                        // delivering key events. Re-enable immediately; if the mach
                        // port isn't available yet, flag for watchdog restart.
                        if matches!(
                            event_type,
                            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
                        ) {
                            log::warn!(
                                "CGEventTap disabled by macOS: {:?} — attempting re-enable",
                                event_type
                            );
                            let port = *port_slot_cb.lock();
                            match port {
                                Some(MachPortHandle(port)) => {
                                    // SAFETY: `port` is the mach port of a CGEventTap
                                    // that is still alive (owned by this thread's run
                                    // loop). `CGEventTapEnable` is thread-safe.
                                    unsafe { CGEventTapEnable(port, true) };
                                    log::info!("CGEventTap re-enabled from callback");
                                }
                                None => {
                                    log::error!(
                                        "CGEventTap disabled but mach port not yet stored \
                                         — flagging for watchdog restart"
                                    );
                                    needs_restart_cb.store(true, Ordering::SeqCst);
                                }
                            }
                            return;
                        }

                        let flags = event.get_flags();
                        let fn_pressed = (flags.bits() & SECONDARY_FN_FLAG) != 0;
                        let was_pressed = fn_was_pressed_clone.load(Ordering::SeqCst);

                        match mode {
                            ActivationMode::HoldFn => {
                                if fn_pressed && !was_pressed {
                                    log::info!("Fn key down - starting recording");
                                    let _ = tx_clone.send(RecordingCommand::Start);
                                } else if !fn_pressed && was_pressed {
                                    log::info!("Fn key up - stopping recording");
                                    let _ = tx_clone.send(RecordingCommand::Stop);
                                }
                            }
                            ActivationMode::TapFn => {
                                if fn_pressed && !was_pressed {
                                    log::info!("Fn tap - toggle recording");
                                    let _ = tx_clone.send(RecordingCommand::Toggle);
                                }
                            }
                            ActivationMode::DoubleTapFn => {
                                if fn_pressed && !was_pressed {
                                    let now = Instant::now();
                                    let mut last = last_tap_time_clone.lock();
                                    let is_double_tap = last
                                        .map(|t| now.duration_since(t).as_millis() < 300)
                                        .unwrap_or(false);

                                    if is_double_tap {
                                        let currently_recording =
                                            is_recording_clone.load(Ordering::SeqCst);
                                        let new_state = !currently_recording;
                                        is_recording_clone.store(new_state, Ordering::SeqCst);
                                        if new_state {
                                            log::info!("Fn double-tap - start");
                                            let _ = tx_clone.send(RecordingCommand::Start);
                                        } else {
                                            log::info!("Fn double-tap - stop");
                                            let _ = tx_clone.send(RecordingCommand::Stop);
                                        }
                                        *last = None;
                                    } else {
                                        *last = Some(now);
                                    }
                                }
                            }
                            ActivationMode::Shortcut => {
                                // Handled by tauri-plugin-global-shortcut instead
                            }
                        }

                        fn_was_pressed_clone.store(fn_pressed, Ordering::SeqCst);
                    }));

                    if result.is_err() {
                        log::error!("Panic caught in CGEventTap callback — ignoring event");
                    }

                    None // Don't modify events
                },
            );

            match tap {
                Ok(tap) => unsafe {
                    // Store the mach port so the callback can re-enable the tap if
                    // macOS disables it later. Done before `tap.enable()` so the
                    // first disable event has access to the port.
                    *port_slot.lock() = Some(MachPortHandle(tap.mach_port.as_concrete_TypeRef()));

                    let loop_source = match tap.mach_port.create_runloop_source(0) {
                        Ok(source) => source,
                        Err(()) => {
                            log::error!("Failed to create run loop source for CGEventTap");
                            let _ = ready_tx.send(false);
                            return;
                        }
                    };
                    let run_loop = CFRunLoop::get_current();

                    // Store the run loop so external code can stop it
                    *run_loop_slot_clone.lock() = Some(run_loop.clone());

                    run_loop.add_source(&loop_source, kCFRunLoopCommonModes);
                    tap.enable();
                    active_clone.store(true, Ordering::SeqCst);
                    let _ = ready_tx.send(true);
                    CFRunLoop::run_current();
                    active_clone.store(false, Ordering::SeqCst);
                    log::info!("Fn key monitor thread exiting");
                },
                Err(()) => {
                    log::error!(
                        "Failed to create CGEventTap. \
                         Input Monitoring permission may not be granted."
                    );
                    let _ = ready_tx.send(false);
                    events::emit_event(
                        &app,
                        event_names::PERMISSIONS_STATUS,
                        events::PermissionsPayload {
                            microphone: crate::permissions::microphone_authorization_status()
                                == crate::permissions::MicrophoneAuthStatus::Authorized,
                            accessibility: crate::permissions::is_accessibility_trusted(),
                            input_monitoring: false,
                        },
                    );
                }
            }
        })
        .expect("Failed to spawn fn-key-monitor thread");

    // Wait for the thread to report whether the CGEventTap was created.
    // Timeout after 5 s to avoid hanging if the thread panics before sending.
    let tap_ok = ready_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or(false);

    (
        FnKeyMonitorHandle {
            run_loop: run_loop_slot,
            active,
            needs_restart,
        },
        tap_ok,
    )
}

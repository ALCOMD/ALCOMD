use std::sync::Mutex;

use alcomd_protocol::RpcError;
use tauri::{Emitter, EventTarget, Manager, State, Window, WindowEvent};

const MAIN_WINDOW: &str = "main";
const STATE_EVENT: &str = "alcomd-window-state";
const ERROR_EVENT: &str = "alcomd-window-state-error";
const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 480.0;
const MAX_LOGICAL_SIZE: f64 = 32_768.0;

type Geometry = (f64, f64, bool);

#[derive(Clone, Copy, Debug, PartialEq)]
struct NormalSize {
    width: f64,
    height: f64,
}

impl NormalSize {
    fn new(width: f64, height: f64) -> Option<Self> {
        (width.is_finite()
            && height.is_finite()
            && width > 0.0
            && height > 0.0
            && width <= MAX_LOGICAL_SIZE
            && height <= MAX_LOGICAL_SIZE)
            .then_some(Self { width, height })
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    size: NormalSize,
    maximized: bool,
    minimized: bool,
    fullscreen: bool,
}

#[derive(Default)]
struct WindowMemory {
    configured: bool,
    enabled: bool,
    normal: Option<NormalSize>,
    maximized: bool,
    restoring: bool,
}

impl WindowMemory {
    fn configure(&mut self, enabled: bool) -> bool {
        let first = !self.configured;
        self.configured = true;
        self.enabled = enabled;
        if !enabled {
            self.restoring = false;
        }
        first && enabled
    }

    fn prepare_restore(&mut self, geometry: Geometry) {
        self.normal = Some(NormalSize {
            width: geometry.0,
            height: geometry.1,
        });
        self.maximized = geometry.2;
        self.restoring = true;
    }

    fn finish_restore(&mut self, snapshot: Snapshot) -> Option<Geometry> {
        self.restoring = false;
        let _ = self.observe(snapshot);
        self.geometry()
    }

    fn geometry(&self) -> Option<Geometry> {
        self.normal
            .map(|size| (size.width, size.height, self.maximized))
    }

    fn observe(&mut self, snapshot: Snapshot) -> Option<Geometry> {
        if self.restoring || snapshot.minimized || snapshot.fullscreen {
            return None;
        }
        if !snapshot.maximized {
            self.normal = Some(snapshot.size);
        }
        self.maximized = snapshot.maximized;
        if self.enabled { self.geometry() } else { None }
    }
}

/// Disposable native geometry for the official main window only.
#[derive(Default)]
pub(crate) struct WindowStateController {
    memory: Mutex<WindowMemory>,
}

fn unavailable() -> RpcError {
    window_failure(std::io::Error::other("window geometry is unavailable"))
}
fn window_failure(error: impl std::error::Error) -> RpcError {
    let id =
        super::local_log::record_native_error(&error, "window state could not be read or restored");
    RpcError {
        code: "window_state_unavailable".to_owned(),
        message: "The window state could not be read or restored.".to_owned(),
        diagnostic_id: Some(id),
        data: None,
    }
}

fn cached_geometry(
    width: Option<f64>,
    height: Option<f64>,
    maximized: Option<bool>,
) -> Result<Option<Geometry>, RpcError> {
    match (width, height, maximized) {
        (None, None, None) => Ok(None),
        (Some(width), Some(height), Some(maximized))
            if width >= MIN_WIDTH && height >= MIN_HEIGHT =>
        {
            NormalSize::new(width, height)
                .map(|size| Some((size.width, size.height, maximized)))
                .ok_or_else(RpcError::invalid_request)
        }
        _ => Err(RpcError::invalid_request()),
    }
}

fn read_snapshot(window: &Window) -> Result<Snapshot, RpcError> {
    let scale = window.scale_factor().map_err(window_failure)?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(unavailable());
    }
    let physical = window.inner_size().map_err(window_failure)?;
    let minimized = window.is_minimized().map_err(window_failure)?;
    let maximized = window.is_maximized().map_err(window_failure)?;
    let fullscreen = window.is_fullscreen().map_err(window_failure)?;
    // Minimized windows may report a zero client area; that size is never recorded.
    let size = if minimized {
        NormalSize {
            width: MIN_WIDTH,
            height: MIN_HEIGHT,
        }
    } else {
        NormalSize::new(
            f64::from(physical.width) / scale,
            f64::from(physical.height) / scale,
        )
        .ok_or_else(unavailable)?
    };
    Ok(Snapshot {
        size,
        maximized,
        minimized,
        fullscreen,
    })
}

struct RestoreBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
    frame_width: f64,
    frame_height: f64,
}

struct RestorePlan {
    geometry: Geometry,
    x: i32,
    y: i32,
}

impl RestoreBounds {
    fn plan(&self, geometry: Geometry) -> Result<RestorePlan, RpcError> {
        if !self.scale.is_finite() || self.scale <= 0.0 || self.width <= 0.0 || self.height <= 0.0 {
            return Err(unavailable());
        }
        let max_width = ((self.width - self.frame_width) / self.scale).max(MIN_WIDTH);
        let max_height = ((self.height - self.frame_height) / self.scale).max(MIN_HEIGHT);
        let width = geometry.0.clamp(MIN_WIDTH, max_width);
        let height = geometry.1.clamp(MIN_HEIGHT, max_height);
        Ok(RestorePlan {
            geometry: (width, height, geometry.2),
            x: (self.x + (self.width - width * self.scale - self.frame_width) / 2.0).round() as i32,
            y: (self.y + (self.height - height * self.scale - self.frame_height) / 2.0).round()
                as i32,
        })
    }
}

fn restore_plan(window: &Window, geometry: Geometry) -> Result<RestorePlan, RpcError> {
    let monitor = window
        .current_monitor()
        .map_err(window_failure)?
        .ok_or_else(unavailable)?;
    let inner = window.inner_size().map_err(window_failure)?;
    let outer = window.outer_size().map_err(window_failure)?;
    let area = monitor.work_area();
    RestoreBounds {
        x: f64::from(area.position.x),
        y: f64::from(area.position.y),
        width: f64::from(area.size.width),
        height: f64::from(area.size.height),
        scale: monitor.scale_factor(),
        frame_width: f64::from(outer.width.saturating_sub(inner.width)),
        frame_height: f64::from(outer.height.saturating_sub(inner.height)),
    }
    .plan(geometry)
}

fn apply_restore(window: &Window, plan: &RestorePlan, maximized: bool) -> Result<(), RpcError> {
    if maximized {
        window.unmaximize().map_err(window_failure)?;
    }
    window
        .set_size(tauri::LogicalSize::new(plan.geometry.0, plan.geometry.1))
        .map_err(window_failure)?;
    window
        .set_position(tauri::PhysicalPosition::new(plan.x, plan.y))
        .map_err(window_failure)?;
    if plan.geometry.2 {
        window.maximize().map_err(window_failure)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn gui_window_state_configure(
    window: Window,
    state: State<'_, WindowStateController>,
    enabled: bool,
    fullscreen: bool,
    width: Option<f64>,
    height: Option<f64>,
    maximized: Option<bool>,
) -> Result<Option<Geometry>, RpcError> {
    if window.label() != MAIN_WINDOW {
        return Err(RpcError::invalid_request());
    }
    let cached = cached_geometry(width, height, maximized)?;
    let first_enable = state
        .memory
        .lock()
        .map_err(window_failure)?
        .configure(enabled);
    let result = (|| {
        if enabled {
            configure_enabled(&window, &state, first_enable, cached)?;
        }
        configure_fullscreen(&window, &state, fullscreen)?;
        Ok(if enabled {
            state.memory.lock().map_err(window_failure)?.geometry()
        } else {
            None
        })
    })();
    if result.is_err()
        && let Ok(mut memory) = state.memory.lock()
    {
        memory.configure(false);
    }
    result
}

fn configure_fullscreen(
    window: &Window,
    state: &WindowStateController,
    fullscreen: bool,
) -> Result<(), RpcError> {
    let snapshot = read_snapshot(window)?;
    if snapshot.fullscreen == fullscreen {
        return Ok(());
    }
    {
        let mut memory = state.memory.lock().map_err(window_failure)?;
        let _ = memory.observe(snapshot);
        // Ignore transition sizes; fullscreen must never replace normal geometry.
        memory.restoring = true;
    }
    let result = window.set_fullscreen(fullscreen).map_err(window_failure);
    let observed = read_snapshot(window);
    let mut memory = state.memory.lock().map_err(window_failure)?;
    memory.restoring = false;
    if let Ok(snapshot) = observed.as_ref() {
        let _ = memory.observe(*snapshot);
    }
    result?;
    observed.map(|_| ())
}

fn configure_enabled(
    window: &Window,
    state: &WindowStateController,
    first_enable: bool,
    cached: Option<Geometry>,
) -> Result<Option<Geometry>, RpcError> {
    let snapshot = read_snapshot(window)?;
    if let Some(cached) = cached.filter(|_| first_enable && !snapshot.fullscreen) {
        let plan = restore_plan(window, cached)?;
        state
            .memory
            .lock()
            .map_err(window_failure)?
            .prepare_restore(plan.geometry);
        apply_restore(window, &plan, snapshot.maximized)?;
        // Dispatcher getters follow the restore requests. Release protection using the
        // actual result, including any size adjustment made by the window manager.
        let restored = read_snapshot(window)?;
        return Ok(state
            .memory
            .lock()
            .map_err(window_failure)?
            .finish_restore(restored));
    }
    let fallback = window
        .app_handle()
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == MAIN_WINDOW)
        .and_then(|config| NormalSize::new(config.width, config.height))
        .ok_or_else(unavailable)?;
    let mut memory = state.memory.lock().map_err(window_failure)?;
    memory.normal.get_or_insert(fallback);
    let _ = memory.observe(snapshot);
    Ok(memory.geometry())
}

pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW
        || !matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        )
    {
        return;
    }
    if let Err(error) = record_window_event(window) {
        let state = window.state::<WindowStateController>();
        let enabled = state.memory.lock().is_ok_and(|memory| memory.enabled);
        if enabled {
            // A closing/destroyed window need not deliver this best-effort error event.
            let _ = window.emit_to(EventTarget::window(MAIN_WINDOW), ERROR_EVENT, error);
        }
    }
}

fn record_window_event(window: &Window) -> Result<(), RpcError> {
    let snapshot = read_snapshot(window)?;
    let state = window.state::<WindowStateController>();
    let geometry = state
        .memory
        .lock()
        .map_err(window_failure)?
        .observe(snapshot);
    if let Some(geometry) = geometry {
        window
            .emit_to(EventTarget::window(MAIN_WINDOW), STATE_EVENT, geometry)
            .map_err(window_failure)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(width: f64, height: f64, maximized: bool) -> Snapshot {
        Snapshot {
            size: NormalSize { width, height },
            maximized,
            minimized: false,
            fullscreen: false,
        }
    }

    #[test]
    fn cache_requires_complete_reasonable_finite_geometry() {
        assert_eq!(cached_geometry(None, None, None).expect("empty"), None);
        assert!(cached_geometry(Some(800.0), None, Some(false)).is_err());
        for width in [0.0, -1.0, 319.0, f64::NAN, f64::INFINITY, 32_769.0] {
            assert!(cached_geometry(Some(width), Some(600.0), Some(false)).is_err());
        }
        assert!(cached_geometry(Some(800.0), Some(f64::NEG_INFINITY), Some(false)).is_err());
        assert!(cached_geometry(Some(800.0), Some(479.0), Some(false)).is_err());
    }

    #[test]
    fn restore_is_allowed_only_on_the_first_configuration() {
        let mut memory = WindowMemory::default();
        assert!(memory.configure(true));
        assert!(!memory.configure(true));
        assert!(!memory.configure(false));
        assert!(!memory.configure(true));

        let mut initially_disabled = WindowMemory::default();
        assert!(!initially_disabled.configure(false));
        assert!(!initially_disabled.configure(true));
    }

    #[test]
    fn maximize_minimize_and_fullscreen_keep_the_normal_size() {
        let mut memory = WindowMemory::default();
        memory.configure(true);
        assert_eq!(
            memory.observe(snapshot(900.0, 600.0, false)),
            Some((900.0, 600.0, false))
        );
        assert_eq!(
            memory.observe(snapshot(1920.0, 1080.0, true)),
            Some((900.0, 600.0, true))
        );
        let mut minimized = snapshot(0.0, 0.0, false);
        minimized.minimized = true;
        assert_eq!(memory.observe(minimized), None);
        assert_eq!(memory.geometry(), Some((900.0, 600.0, true)));
        let mut fullscreen = snapshot(2560.0, 1440.0, false);
        fullscreen.fullscreen = true;
        assert_eq!(memory.observe(fullscreen), None);
        assert_eq!(memory.geometry(), Some((900.0, 600.0, true)));
        assert_eq!(
            memory.observe(snapshot(1000.0, 700.0, false)),
            Some((1000.0, 700.0, false))
        );
    }

    #[test]
    fn restore_events_do_not_replace_the_requested_normal_size() {
        let mut memory = WindowMemory::default();
        memory.configure(true);
        memory.prepare_restore((1000.0, 700.0, true));
        assert_eq!(memory.observe(snapshot(1400.0, 800.0, false)), None);
        assert_eq!(
            memory.finish_restore(snapshot(1920.0, 1080.0, true)),
            Some((1000.0, 700.0, true))
        );
        assert_eq!(
            memory.observe(snapshot(1000.0, 700.0, false)),
            Some((1000.0, 700.0, false))
        );

        memory.prepare_restore((850.0, 650.0, false));
        assert_eq!(memory.observe(snapshot(1000.0, 700.0, false)), None);
        assert_eq!(
            memory.finish_restore(snapshot(840.0, 640.0, false)),
            Some((840.0, 640.0, false))
        );
        assert_eq!(
            memory.observe(snapshot(950.0, 700.0, false)),
            Some((950.0, 700.0, false))
        );
    }

    #[test]
    fn disabled_memory_tracks_normal_size_without_emitting() {
        let mut memory = WindowMemory::default();
        assert_eq!(memory.observe(snapshot(900.0, 600.0, false)), None);
        memory.configure(true);
        assert_eq!(
            memory.observe(snapshot(1920.0, 1080.0, true)),
            Some((900.0, 600.0, true))
        );
        memory.configure(false);
        assert_eq!(memory.observe(snapshot(1100.0, 750.0, false)), None);
        assert_eq!(memory.geometry(), Some((1100.0, 750.0, false)));
    }

    #[test]
    fn restore_fits_the_current_work_area_at_its_scale_and_centers() {
        let bounds = RestoreBounds {
            x: -2560.0,
            y: 0.0,
            width: 2560.0,
            height: 1400.0,
            scale: 2.0,
            frame_width: 16.0,
            frame_height: 64.0,
        };
        let plan = bounds.plan((3000.0, 2000.0, true)).expect("valid area");
        assert_eq!(plan.geometry, (1272.0, 668.0, true));
        assert_eq!((plan.x, plan.y), (-2560, 0));
        let small = bounds.plan((100.0, 100.0, false)).expect("minimum");
        assert_eq!(small.geometry, (320.0, 480.0, false));
        assert_eq!((small.x, small.y), (-1608, 188));
    }
}

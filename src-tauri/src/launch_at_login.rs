//! Launch-at-login via macOS ServiceManagement (SMAppService, macOS 13+).
//!
//! Thin wrapper over `smappservice-rs` that maps the framework's
//! `ServiceStatus` into a serializable enum the frontend can react to,
//! and exposes register/unregister with no business logic.

use smappservice_rs::{AppService, ServiceStatus, ServiceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchAtLoginStatus {
    Enabled,
    NotRegistered,
    RequiresApproval,
    NotFound,
}

impl From<ServiceStatus> for LaunchAtLoginStatus {
    fn from(s: ServiceStatus) -> Self {
        match s {
            ServiceStatus::Enabled => Self::Enabled,
            ServiceStatus::NotRegistered => Self::NotRegistered,
            ServiceStatus::RequiresApproval => Self::RequiresApproval,
            ServiceStatus::NotFound => Self::NotFound,
        }
    }
}

pub fn status() -> LaunchAtLoginStatus {
    AppService::new(ServiceType::MainApp).status().into()
}

pub fn set_enabled(enabled: bool) -> Result<LaunchAtLoginStatus, String> {
    let svc = AppService::new(ServiceType::MainApp);
    if enabled {
        svc.register().map_err(|e| e.to_string())?;
    } else {
        // unregister() errors when not registered; treat as idempotent.
        let _ = svc.unregister();
    }
    Ok(svc.status().into())
}

pub fn open_login_items_settings() {
    AppService::open_system_settings_login_items();
}

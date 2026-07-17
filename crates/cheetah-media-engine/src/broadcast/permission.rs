//! Permission model for capture sources.
//!
//! WP-71 defines the permission surface. Real platform permission dialogs will
//! be wired in later work packages; the host model here is honest about having
//! no capture hardware and returns `Denied` rather than faking success.

/// Kind of capture source that may require user permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CaptureSourceKind {
    /// Video camera.
    Camera,
    /// Audio microphone.
    #[default]
    Microphone,
    /// Entire screen or a window.
    Screen,
    /// Specific application content (e.g. browser tab).
    Application,
    /// Custom capture source defined by the caller.
    Custom,
}

impl CaptureSourceKind {
    /// Stable string identifier used in diagnostics and registry lookup.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::Screen => "screen",
            Self::Application => "application",
            Self::Custom => "custom",
        }
    }
}

/// State of a platform permission for a capture kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionState {
    /// Not yet queried.
    #[default]
    Unknown,
    /// User should be prompted.
    Prompt,
    /// Permission granted.
    Granted,
    /// Permission denied by user or policy.
    Denied,
    /// Permission restricted by the system (e.g. parental controls, MDM).
    Restricted,
}

/// Permission model queried by the broadcast engine before starting capture.
pub trait PermissionModel: Send {
    /// Query the current permission state without prompting the user.
    fn query(&self, kind: CaptureSourceKind) -> PermissionState;

    /// Prompt the user (or the host platform) for permission.
    ///
    /// Returns the new state; the model is allowed to return `Denied` without
    /// user interaction on platforms where the permission cannot be granted.
    fn request(&mut self, kind: CaptureSourceKind) -> PermissionState;
}

/// Host permission model used when no platform SDK is linked.
///
/// It denies camera and screen access because the host has no real capture
/// hardware, while leaving microphone as `Unknown` to signal that a real model
/// must be installed. This avoids faking support while still allowing tests to
/// inject a granting model.
pub struct HostPermissionModel;

impl PermissionModel for HostPermissionModel {
    fn query(&self, kind: CaptureSourceKind) -> PermissionState {
        match kind {
            CaptureSourceKind::Camera
            | CaptureSourceKind::Screen
            | CaptureSourceKind::Application => PermissionState::Denied,
            CaptureSourceKind::Microphone | CaptureSourceKind::Custom => PermissionState::Unknown,
        }
    }

    fn request(&mut self, kind: CaptureSourceKind) -> PermissionState {
        self.query(kind)
    }
}

/// Permission model that always grants permission; useful for headless tests.
pub struct AlwaysGrantPermissionModel;

impl PermissionModel for AlwaysGrantPermissionModel {
    fn query(&self, _kind: CaptureSourceKind) -> PermissionState {
        PermissionState::Granted
    }

    fn request(&mut self, _kind: CaptureSourceKind) -> PermissionState {
        PermissionState::Granted
    }
}

/// Permission model that always denies permission; useful for fault injection.
pub struct AlwaysDenyPermissionModel;

impl PermissionModel for AlwaysDenyPermissionModel {
    fn query(&self, _kind: CaptureSourceKind) -> PermissionState {
        PermissionState::Denied
    }

    fn request(&mut self, _kind: CaptureSourceKind) -> PermissionState {
        PermissionState::Denied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_model_denies_camera_and_screen() {
        let model = HostPermissionModel;
        assert_eq!(
            model.query(CaptureSourceKind::Camera),
            PermissionState::Denied
        );
        assert_eq!(
            model.query(CaptureSourceKind::Screen),
            PermissionState::Denied
        );
        assert_eq!(
            model.query(CaptureSourceKind::Application),
            PermissionState::Denied
        );
    }

    #[test]
    fn always_grant_and_deny_models() {
        let mut grant = AlwaysGrantPermissionModel;
        let mut deny = AlwaysDenyPermissionModel;
        assert_eq!(
            grant.request(CaptureSourceKind::Microphone),
            PermissionState::Granted
        );
        assert_eq!(
            deny.request(CaptureSourceKind::Camera),
            PermissionState::Denied
        );
    }
}

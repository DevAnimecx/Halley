//! Permission model foundation: typed requests, per-origin decisions.
//!
//! No settings UI yet. Defaults are deny/ask for sensitive capabilities;
//! grants are in-memory and origin-scoped (never a global
//! `camera_allowed = true`).

use std::collections::HashMap;

use halley_common::Origin;

/// Browser capability a page (or Jerry) may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Camera,
    Microphone,
    Location,
    Notifications,
    ClipboardRead,
    ClipboardWrite,
    Fullscreen,
    Downloads,
    Popups,
}

/// Outcome for a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny,
    /// Not yet decided — future UX prompts (default for most).
    Ask,
}

/// A permission request bound to an origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// Origin requesting the capability (must be parsed, not a raw string).
    pub origin: Origin,
    /// Which capability is requested.
    pub permission: Permission,
    /// Whether the request arrived from a private session.
    pub private_mode: bool,
}

impl PermissionRequest {
    /// Build a request (fails if `origin_url` is not http/https).
    pub fn new(
        origin_url: &str,
        permission: Permission,
        private_mode: bool,
    ) -> Result<Self, halley_common::OriginError> {
        Ok(PermissionRequest {
            origin: Origin::parse(origin_url)?,
            permission,
            private_mode,
        })
    }
}

/// Central permission policy owner.
#[derive(Debug, Clone)]
pub struct PermissionPolicy {
    default: PermissionDecision,
    /// Per-origin, per-permission remembered decisions (in-memory only).
    grants: HashMap<(Origin, Permission), PermissionDecision>,
}

impl PermissionPolicy {
    /// Sensitive defaults: camera/mic/location deny; others ask.
    pub fn secure_default() -> Self {
        PermissionPolicy {
            default: PermissionDecision::Ask,
            grants: HashMap::new(),
        }
    }

    fn builtin_default(permission: Permission) -> PermissionDecision {
        match permission {
            Permission::Camera
            | Permission::Microphone
            | Permission::Location
            | Permission::ClipboardRead => PermissionDecision::Deny,
            Permission::Notifications
            | Permission::ClipboardWrite
            | Permission::Fullscreen
            | Permission::Downloads
            | Permission::Popups => PermissionDecision::Ask,
        }
    }

    /// Decide for a request using stored grant or builtin default.
    pub fn decide(&self, request: &PermissionRequest) -> PermissionDecision {
        if let Some(d) = self
            .grants
            .get(&(request.origin.clone(), request.permission))
        {
            return *d;
        }
        Self::builtin_default(request.permission)
    }

    /// Remember a user decision for this origin only.
    pub fn remember(
        &mut self,
        origin: Origin,
        permission: Permission,
        decision: PermissionDecision,
    ) {
        self.grants.insert((origin, permission), decision);
    }

    /// Forget all grants (clear-site-data style).
    pub fn clear_origin(&mut self, origin: &Origin) {
        self.grants.retain(|(o, _), _| o != origin);
    }

    /// Current default when no grant exists (tests / diagnostics).
    pub fn default_decision(&self) -> PermissionDecision {
        self.default
    }
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self::secure_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_denied_by_default_per_origin() {
        let policy = PermissionPolicy::default();
        let a = PermissionRequest::new("https://a.example/", Permission::Camera, false).unwrap();
        let b = PermissionRequest::new("https://b.example/", Permission::Camera, false).unwrap();
        assert_eq!(policy.decide(&a), PermissionDecision::Deny);
        assert_eq!(policy.decide(&b), PermissionDecision::Deny);
    }

    #[test]
    fn allow_is_scoped_to_one_origin() {
        let mut policy = PermissionPolicy::default();
        let origin_a = Origin::parse("https://a.example/").unwrap();
        let _origin_b = Origin::parse("https://b.example/").unwrap();
        policy.remember(
            origin_a,
            Permission::Notifications,
            PermissionDecision::Allow,
        );
        let a =
            PermissionRequest::new("https://a.example/", Permission::Notifications, false).unwrap();
        let b =
            PermissionRequest::new("https://b.example/", Permission::Notifications, false).unwrap();
        assert_eq!(policy.decide(&a), PermissionDecision::Allow);
        assert_eq!(policy.decide(&b), PermissionDecision::Ask);
    }

    #[test]
    fn no_global_allow_flag_exists_without_origin() {
        // API requires an Origin — cannot express global camera allow.
        let policy = PermissionPolicy::default();
        assert_eq!(policy.default_decision(), PermissionDecision::Ask);
    }
}

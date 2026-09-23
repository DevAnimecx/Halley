//! Workspace smoke tests.
//!
//! Proves the virtual Cargo workspace builds, links, and that shared
//! configuration behaves as documented. These are real assertions over real
//! code — no browser functionality is simulated here.

#[test]
fn core_declares_its_subsystem_identity() {
    assert_eq!(halley_core::SUBSYSTEM, "halley-core");
}

#[test]
fn shared_config_defaults_are_privacy_preserving() {
    let config = halley_common::Config::default();
    assert!(!config.privacy.telemetry_enabled);
    assert!(config.privacy.block_trackers);
    assert!(!config.jerry.enabled);
    assert!(config.jerry.require_confirmation_for_destructive);
    assert!(config.ai.provider.is_empty());
}

#[test]
fn shared_error_strategy_scopes_messages_by_subsystem() {
    let err = halley_common::Error::subsystem(halley_core::SUBSYSTEM, "not implemented yet");
    assert_eq!(err.to_string(), "[halley-core] not implemented yet");
}

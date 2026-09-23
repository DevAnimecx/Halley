# Assets

Static assets (icons, images, filter-list snapshots, locale files).

- `chrome.html` — embedded browser chrome (tab strip + address bar +
  back/forward/reload) loaded into the chrome web view by `halley-engine`.
  Trusted, compiled into the binary via `include_str!`. No network
  requests: no external fonts, scripts, or images. Hosts chrome-scoped
  keyboard shortcuts (see ADR-007 for content-focus limits).

Nothing in this directory may contain secrets or user data.

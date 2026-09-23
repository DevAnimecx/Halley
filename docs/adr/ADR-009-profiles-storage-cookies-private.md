# ADR-009: Profiles, storage isolation, cookie ownership, private browsing

- Status: **IMPLEMENTED** (layout + policy layer; page-cookie sync with the
  platform WebView is **NOT IMPLEMENTED**)

## Context

Prompt #4 requires a normal vs private profile split, storage layout with
path-safety, centralized cookie/storage/cache/referrer/permission policy,
and private-mode cleanup — without leaking cookies across profiles or
escaping the profile root via crafted names.

## Decision

- **Two profile kinds**: `Normal` (persistent, under
  `Config.storage.profile_root`) and `Private` (ephemeral, unique root
  `halley-private-{pid}-{seq}` under the system temp dir).
- `BrowserProfile::close()` deletes the entire private root; normal close
  leaves user data. Private roots are refused if nested under the normal
  profile root (`ProfileError::NotAllowed`).
- Storage categories are strictly separated and never share files:
  `profile | cache | cookies | session | logs | temp`.
- All path construction goes through `sanitize_component` /
  `safe_join_component`: rejects `..`, `/`, `\`, `:`, NUL, controls, and
  encoded traversal (`%2e`, `%2f`, `%5c`, `%00`). Untrusted input never
  becomes a raw path segment.
- **Cookie ownership (honest split)**:
  - `halley-privacy::CookieStore` is the **policy layer**: parses
    `Set-Cookie` via the `cookie` crate, enforces domain/path matching,
    `Secure`, `HttpOnly`, `SameSite`, expiry, third-party block
    (`block_third_party: true` by default), private-mode ephemeral jars.
  - The **platform WebView still owns live page cookies** for documents it
    loads. Syncing WebView cookies into `CookieStore` is **NOT
    IMPLEMENTED** and is tracked as future work (engine interception
    milestone). Jerry/extension-facing APIs must use `CookieStore`, not
    reach into the webview.
- Private and normal jars are in-memory separated per profile instance;
  private jars are never written to the normal cookie file.
- First/third party is approximated with `site_label` (last two dot
  labels) in `halley-common` — **no Public Suffix List** yet (documented
  limitation).
- Permissions: `camera | microphone | geolocation | clipboard-read` deny
  by default; others `ask`; grants keyed by `(Origin, Permission)` in the
  profile. UI for prompting is **NOT IMPLEMENTED**.

## Reason

Isolation must be structural (paths, separate roots), not convention-only.
Cookie *policy* belongs with privacy; cookie *storage inside pages* is a
platform concern until interception exists — documenting both avoids
faking a full cookie jar that the webview does not use.

## Consequences

- Tests prove layout, traversal rejection, in-memory jar isolation, and
  private cleanup; they cannot yet prove WebView page-cookie deletion on
  private close (gap stated in docs + final report).
- `site_label` mismatches exotic multi-part public suffixes (e.g. some
  `co.uk`-style registrable domains) until PSL lands.
- Referrer stripping is policy-computation only until the engine applies
  headers on navigation.

## Alternatives

- Platform cookie APIs per profile now: rejected — wry/webview cookie
  enumeration differs per OS and is engine-work, not policy-work.
- Storing private data under `profile_root` with a flag: rejected — a bug
  would persist private data beside real profiles; temp root + full delete
  is safer.

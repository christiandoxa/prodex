# Mojo ownership of imported profile identity email resolution

This checkpoint moves the deterministic email choice in
`resolve_profile_import_identity` into operation 21 of the existing
`prodex_mojo_profile_identity_v2` ABI. A present auth email is copied
verbatim, including an empty value. When auth has no email, Mojo trims the
exported fallback with the existing Unicode trim rules and drops it when
empty.

Rust keeps `ProfileImportIdentity` mutation, UTF-8/string materialization,
error handling, credentials, and filesystem effects. The profile-export
caller regression and required-Mojo ABI tests cover precedence, Unicode
whitespace, empty fallback, empty auth email, and invalid flags.

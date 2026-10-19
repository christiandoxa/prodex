# Kiro ACP session/update parsing policy migrated to compiled Mojo

The live Kiro Agent Client Protocol session/update parser now delegates
its JSON discriminant classification and every update-specific required-
field decision to one standalone compiled Mojo domain owner. Rust still
uses Serde to acquire and type JSON values and materializes the selected
Rust enum variants; the previous string-discriminator match and
independent required-field checks were deleted.

The versioned compiled policy handles user/agent/thought message chunks,
tool calls, tool-call updates, plan entries, usage updates, session
information, available commands, current mode, and unknown future update
types. The discriminator is matched by exact UTF-8 bytes; unknown future
types preserve the original raw JSON. Missing/incorrect field types use
the same observable error precedence, such as toolCallId before title
and used before size, with validation performed only by Mojo.

The host sends a compact mask of Serde-acquired field presence/type facts
and a borrowed sessionUpdate string. The Mojo ABI validates version,
UTF-8 shape, mask, pointers, output capacity, and writes a kind plus the
first selected field error. The Rust host maps that error to stable
public text and treats any Mojo ABI failure as an explicit error; it does
not re-run a Rust classifier. Required-field values are only accessed
after the compiled owner has verified their presence and shape.

Three direct ABI tests cover all known update variants, unknown Unicode
discriminators, missing fields, precedence, invalid UTF-8, malformed ABI
arguments and no writes on rejected shapes. Two new caller regressions
exercise Serde-to-Mojo-to-Rust conversion and the original error messages.
All 32 Kiro runtime and 686 Mojo-core tests pass, and the no-Rust-fallback
guard enforces delegation and the compiled build source.

This preserves production session and continuation handling without
claiming the separate 75% Mojo production-share goal has been met.

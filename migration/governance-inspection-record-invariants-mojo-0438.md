# Governance inspection record invariants migrated to Mojo

The existing governance inspection Mojo module now owns a versioned,
three-mode scalar validator for all inspection record boundaries:
content-location start/end within the u32 interval and ordered
correctly, confidence between 0 and 10,000 basis points, and
per-result caps for findings, tags and reason codes.

The corresponding Rust conditional policies were removed from
ContentLocation::new, InspectionFinding::new and InspectionResult::new.
Rust retains serde/domain DTO ownership, checked u32 materialization,
error mapping and subsequent Mojo classification/sorting decisions.
No Rust fallback recomputation is present.

The ABI rejects unknown modes and incompatible versions before touching
the output. Direct Mojo golden cases cover all boundary edges; the
existing 29 domain tests and new caller regression cover valid/invalid
locations, finding confidence and collection limits.

The broad 75% Mojo production-share target remains outstanding.

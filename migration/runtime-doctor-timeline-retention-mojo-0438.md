# Runtime doctor request timeline policy migrated to Mojo

The production runtime-doctor timeline is now coordinated by two compiled
operations in the existing runtime_doctor_render.mojo semantic owner:

- Timeline append planning retains the twelve newest events per request.
- Latest timeline selection compares the final recorded line indices of
  BTreeMap-ordered request histories and chooses the lexicographically last
  request when indices tie, preserving the existing Rust max_by_key contract.

The Rust MAX_EVENTS constant, length comparison and max_by_key selection
have been deleted. Rust retains parsing of event JSON/logs, storage of
request histories, allocation, and application of validated Mojo indices.
No duplicate Rust algorithm or failure fallback remains.

Direct Mojo ABI tests and caller regression cover exactly twelve events,
eviction at the thirteenth, lexical ties and a genuinely later competing
request. No runtime conversation data or credentials are processed in tests.

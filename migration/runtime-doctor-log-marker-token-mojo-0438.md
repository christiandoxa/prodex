# Runtime doctor log token marker matching in compiled Mojo

Runtime-doctor Smart Context event detection now delegates the complete token
matching decision to the compiled runtime-doctor marker owner. Mojo performs
Unicode-aware whitespace tokenization, strips exactly the legacy ASCII
punctuation set at both token boundaries, and compares complete marker bytes.
Rust now passes borrowed message and marker views to a versioned Mojo ABI;
there is no duplicate Rust tokenization or fallback decision.

The direct strict-ABI test checks invalid versions and pointers plus empty
markers, exact matches, Unicode spaces and prefix/suffix false positives.
The runtime-doctor caller regression exercises punctuation and Unicode log
boundaries, and the original Smart Context summary tests remain available.

This is an incremental production-reachable migration, not a 75% share claim.

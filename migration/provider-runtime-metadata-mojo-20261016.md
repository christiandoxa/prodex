# Provider runtime metadata in Mojo

The provider registry now owns built-in runtime metadata selection, including remote-compaction names, defaults, web-search modes, and image-generation capability. Rust keeps the public default constants used by CLI/app callers, then validates and caches the compiled Mojo metadata result; no Rust metadata table remains in production.

Compiled-Mojo ABI and provider-core registry tests cover every provider record, the local optional URL, Gemini image generation, and Gemini/Copilot remote-compaction names.

# Gemini request field validation in Mojo

The Gemini bridge validation kernel now owns the runtime path-shape policy for flat function tools. A validation mode on `prodex_gemini_bridge_request_kernel_v1` emits the established `tools[index].function.*` errors while scanning the original bounded JSON. Rust keeps JSON parsing, ABI marshalling, public error mapping, and transport effects; the runtime caller no longer rebuilds a second tools array.

Direct compiled-Mojo coverage and provider/app caller tests cover indexed flat-tool errors, wrapped tools, malformed fields, and the existing validation behavior.

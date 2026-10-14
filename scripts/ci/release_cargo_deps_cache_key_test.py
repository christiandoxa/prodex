"""Hermetic tests for version-independent but supply-chain-aware release caches."""

import importlib.util
from pathlib import Path
import unittest

SCRIPT = Path(__file__).with_name("release_cargo_deps_cache_key.py")
SPEC = importlib.util.spec_from_file_location("cargo_deps_key", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CHECKSUM = "a" * 64


def lock(
    version="0.437.10",
    dependency="1.2.3",
    checksum=CHECKSUM,
    source="registry+https://github.com/rust-lang/crates.io-index",
):
    return (f'''version = 4

[[package]]
name = "prodex"
version = "{version}"
dependencies = ["dep"]

[[package]]
name = "dep"
version = "{dependency}"
source = "{source}"
checksum = "{checksum}"

[[package]]
name = "prodex-app"
version = "{version}"
dependencies = ["dep"]
''').encode()


class StableCargoDependencyCacheTests(unittest.TestCase):
    def test_release_version_metadata_does_not_invalidate_third_party_cache(self):
        self.assertEqual(
            MODULE.external_dependencies_fingerprint(lock("0.437.10")),
            MODULE.external_dependencies_fingerprint(lock("0.437.11")),
        )

    def test_external_version_and_checksum_changes_always_invalidate(self):
        original = MODULE.external_dependencies_fingerprint(lock())
        self.assertNotEqual(original, MODULE.external_dependencies_fingerprint(lock(dependency="1.2.4")))
        self.assertNotEqual(original, MODULE.external_dependencies_fingerprint(lock(checksum="b" * 64)))

    def test_external_source_identity_changes_the_fingerprint(self):
        old = MODULE.external_dependencies_fingerprint(lock())
        new = MODULE.external_dependencies_fingerprint(
            lock(source="registry+https://example.com/index")
        )
        self.assertNotEqual(old, new)

    def test_missing_or_invalid_lockfile_is_rejected(self):
        for source in [b"", b"invalid = ", b"[[package]]\nname = \"prodex\"", b"version=4"]:
            with self.subTest(source=source):
                with self.assertRaises(ValueError):
                    MODULE.external_dependencies_fingerprint(source)
        with self.assertRaises(ValueError):
            MODULE.external_dependencies_fingerprint(lock(checksum="bad"))

    def test_package_order_does_not_change_cache_key(self):
        content = lock()
        a = content.index(b"[[package]]\nname = \"prodex\"")
        b = content.index(b"[[package]]\nname = \"dep\"")
        external_first = b"version = 4\n\n" + content[b:content.index(b"[[package]]\nname = \"prodex-app\"")] + b"\n" + content[a:b] + content[content.index(b"[[package]]\nname = \"prodex-app\""):]
        self.assertEqual(
            MODULE.external_dependencies_fingerprint(content),
            MODULE.external_dependencies_fingerprint(external_first),
        )


if __name__ == "__main__":
    unittest.main()

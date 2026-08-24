from std.collections import Dict, List, Set
from std.ffi import CStringSlice
from std.testing import assert_true
from std.utils import Variant


trait Labeled:
    def label(self) -> Int:
        ...


@fieldwise_init
struct Record(Copyable, Labeled):
    var value: Int

    def label(self) -> Int:
        return self.value


def main() raises:
    var text = String("账户🙂e\u0301\0")
    var view = StringSlice(text)
    var values = List[String]()
    values.append(text.copy())
    var counts = Dict[String, Int]()
    counts[text] = 2
    var unique = Set[String]()
    unique.add(text.copy())
    var optional = Optional[Int](counts[text])
    var variant = Variant[Int, String](String("ok"))
    var record = Record(7)
    var c_bytes: InlineArray[UInt8, 3] = [0x6F, 0x6B, 0]
    var c_string = CStringSlice(Span(c_bytes))
    assert_true(view.count_codepoints() == 6)
    assert_true(len(values) == 1 and len(unique) == 1)
    assert_true(optional and optional.unsafe_value() == 2)
    assert_true(variant.isa[String]())
    assert_true(record.label() == 7)
    assert_true(len(c_string) == 2)

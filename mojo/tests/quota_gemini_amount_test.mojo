from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from quota import GeminiBucketNumericPlan, prodex_quota_gemini_bucket_batch, prodex_quota_round_f64
from quota_gemini_amount import GeminiBucketRecord, prodex_quota_gemini_bucket_records_v1


def check(text: StringSlice, present: Int64, has_remaining: Int64, value: Int64) raises:
    var rows_store = alloc(Layout[GeminiBucketRecord](count=1))
    var out_store = alloc(Layout[GeminiBucketNumericPlan](count=1))
    var rows = rows_store.unsafe_ptr()
    var output = out_store.unsafe_ptr()
    rows[] = GeminiBucketRecord(UInt(Int(text.unsafe_ptr())) if present == 1 else UInt(0), Int64(text.byte_length()), Int64(3) if present == 1 else Int64(0), 0, 0.0, 0)
    var r = UInt(Int(rows))
    var o = UInt(Int(output))
    assert prodex_quota_gemini_bucket_records_v1(1, r, 1, o, 1) == 0
    assert output[].remaining_present == has_remaining and output[].remaining == value
    var old_store = alloc(Layout[Int64](count=10))
    var fraction_store = alloc(Layout[Float64](count=1))
    var old = old_store.unsafe_ptr()
    fraction_store.unsafe_ptr()[] = 0.0
    old[unsafe_offset=0] = value
    old[unsafe_offset=1] = Int64(0) if present == 0 else (Int64(1) if has_remaining == 1 else Int64(2))
    old[unsafe_offset=2] = 0
    var source = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(old))
    var destination = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(old.unsafe_offset(3)))
    assert prodex_quota_gemini_bucket_batch(
        source, source.unsafe_offset(1), fraction_store.unsafe_ptr(), source.unsafe_offset(2),
        destination, destination.unsafe_offset(1), destination.unsafe_offset(2), destination.unsafe_offset(3),
        destination.unsafe_offset(4), destination.unsafe_offset(5), destination.unsafe_offset(6), 1,
    ) == 0
    var words = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(o))
    for index in range(7):
        assert old[unsafe_offset=index + 3] == words[unsafe_offset=index]
    dealloc(fraction_store^)
    dealloc(old_store^)
    output[] = GeminiBucketNumericPlan(77,77,77,77,77,77,77)
    assert prodex_quota_gemini_bucket_records_v1(2,r,1,o,1) == 4
    assert prodex_quota_gemini_bucket_records_v1(1,r,1,o,0) == 3
    assert prodex_quota_gemini_bucket_records_v1(1,r+1,1,o,1) == 1
    assert prodex_quota_gemini_bucket_records_v1(1,r,1025,o,1) == 1
    assert output[].remaining == 77 and output[].exhausted == 77
    dealloc(out_store^)
    dealloc(rows_store^)


def main() raises:
    assert prodex_quota_round_f64(1.5) == 2
    assert prodex_quota_round_f64(-1.5) == -2
    assert prodex_quota_round_f64(9223372036854775808.0) == 9223372036854775807
    check("",0,0,0)
    check("",1,0,0)
    check("　+42\t",1,1,42)
    check("-9223372036854775808",1,1,-9223372036854775808)
    check("9223372036854775807",1,1,9223372036854775807)
    check("9223372036854775808",1,0,0)
    check("-9223372036854775809",1,0,0)
    check("4 2",1,0,0)
    check("+",1,0,0)
    assert prodex_quota_gemini_bucket_records_v1(1,0,0,0,0) == 0

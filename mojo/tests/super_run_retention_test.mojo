from std.memory.alloc import alloc, dealloc, Layout
from super_expose_run_policy import SuperRunRetentionRow, prodex_super_run_retention_v1


def main() raises:
    var row_store=alloc(Layout[SuperRunRetentionRow](count=35))
    var keep_store=alloc(Layout[UInt8](count=35))
    var scratch_store=alloc(Layout[Int64](count=35))
    var rows=row_store.unsafe_ptr()
    var keep=keep_store.unsafe_ptr()
    var scratch=scratch_store.unsafe_ptr()
    var r=UInt(Int(rows))
    var k=UInt(Int(keep))
    var s=UInt(Int(scratch))
    rows[unsafe_offset=0]=SuperRunRetentionRow(0,0,0,0)
    rows[unsafe_offset=1]=SuperRunRetentionRow(1,9,4,1)
    rows[unsafe_offset=2]=SuperRunRetentionRow(1,1,4,1)
    rows[unsafe_offset=3]=SuperRunRetentionRow(1,2,0,0)
    rows[unsafe_offset=4]=SuperRunRetentionRow(1,18446744073709551615,18446744073709551615,1)
    for index in range(5,35):
        rows[unsafe_offset=index]=SuperRunRetentionRow(1,18446744073709551615,18446744073709551614,1)
    assert prodex_super_run_retention_v1(1,r,35,k,35,s,35)==0
    assert keep[unsafe_offset=0]==1 and keep[unsafe_offset=1]==1 and keep[unsafe_offset=2]==0 and keep[unsafe_offset=3]==0 and keep[unsafe_offset=4]==1
    assert prodex_super_run_retention_v1(1,r,5,k,35,s,35)==0
    for index in range(5):
        assert keep[unsafe_offset=index]==1
        keep[unsafe_offset=index]=7
        scratch[unsafe_offset=index]=77
    assert prodex_super_run_retention_v1(2,r,5,k,5,s,5)==4
    assert prodex_super_run_retention_v1(1,r,5,k,4,s,5)==3
    assert prodex_super_run_retention_v1(1,r+1,5,k,5,s,5)==1
    rows[unsafe_offset=4]=SuperRunRetentionRow(2,0,0,0)
    assert prodex_super_run_retention_v1(1,r,5,k,5,s,5)==1
    for index in range(5):
        assert keep[unsafe_offset=index]==7 and scratch[unsafe_offset=index]==77
    assert prodex_super_run_retention_v1(1,0,0,0,0,0,0)==0
    dealloc(scratch_store^)
    dealloc(keep_store^)
    dealloc(row_store^)

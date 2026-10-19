# Caller-owned index heapsort shared by text grouping and binding retention.
from std.memory import Pointer


def index_order_swap(indices: Pointer[mut=True, Int64, MutUntrackedOrigin], left: Int64, right: Int64):
    var value = indices[unsafe_offset=left]
    indices[unsafe_offset=left] = indices[unsafe_offset=right]
    indices[unsafe_offset=right] = value


def index_order_sift[compare: def(UInt, Int64, Int64, Int64) thin -> Int64](
    data: UInt, width: Int64, indices: Pointer[mut=True, Int64, MutUntrackedOrigin], root_index: Int64, end: Int64,
):
    var root = root_index
    while True:
        var child = root * 2 + 1
        if child > end:
            break
        if child + 1 <= end and compare(data, width, indices[unsafe_offset=child], indices[unsafe_offset=child + 1]) < 0:
            child += 1
        if compare(data, width, indices[unsafe_offset=root], indices[unsafe_offset=child]) >= 0:
            break
        index_order_swap(indices, root, child)
        root = child


def index_order_sort[compare: def(UInt, Int64, Int64, Int64) thin -> Int64](
    data: UInt, width: Int64, indices: Pointer[mut=True, Int64, MutUntrackedOrigin], count: Int64,
):
    var start = count // 2
    while start > 0:
        start -= 1
        index_order_sift[compare](data, width, indices, start, count - 1)
    var end = count
    while end > 1:
        end -= 1
        index_order_swap(indices, 0, end)
        index_order_sift[compare](data, width, indices, 0, end - 1)

from std.os import getenv
from std.runtime.asyncrt import create_task
from std.subprocess import run
from std.testing import assert_true


async def async_value() -> Int:
    return 7


def main() raises:
    var path = getenv("PATH")
    assert_true(path)
    with open("/dev/null", "r") as handle:
        assert_true(handle.read() == "")
    assert_true(run("printf prodex") == "prodex")
    var task = create_task(async_value())
    assert_true(task^.wait() == 7)

from confirmation_policy import prodex_manual_redeem_reset_v1
from runtime_math import runtime_i64_saturating_sub


def main() raises:
    assert prodex_manual_redeem_reset_v1(1,0,0,0,0,1000,3600) == 0
    assert prodex_manual_redeem_reset_v1(1,1,4601,1,604800,1000,3600) == 0
    assert prodex_manual_redeem_reset_v1(1,1,4600,1,604800,1000,3600) == 1
    assert prodex_manual_redeem_reset_v1(1,1,1060,1,1030,1000,3600) == 2
    assert prodex_manual_redeem_reset_v1(1,1,1030,1,1030,1000,3600) == 1
    assert prodex_manual_redeem_reset_v1(1,0,0,1,0,1000,3600) == 2
    assert prodex_manual_redeem_reset_v1(1,1,800,1,900,1000,-100) == 1
    assert prodex_manual_redeem_reset_v1(1,1,9223372036854775807,0,0,-9223372036854775808,9223372036854775807) == 1
    assert prodex_manual_redeem_reset_v1(1,1,9223372036854775807,0,0,-9223372036854775808,9223372036854775806) == 0
    assert prodex_manual_redeem_reset_v1(1,1,-9223372036854775808,0,0,9223372036854775807,-9223372036854775808) == 1
    assert runtime_i64_saturating_sub(-9223372036854775808,9223372036854775807) == -9223372036854775808
    assert runtime_i64_saturating_sub(9223372036854775807,-9223372036854775808) == 9223372036854775807
    assert prodex_manual_redeem_reset_v1(2,0,0,0,0,0,0) == -2
    assert prodex_manual_redeem_reset_v1(1,2,0,0,0,0,0) == -1
    assert prodex_manual_redeem_reset_v1(1,0,1,0,0,0,0) == -1
    assert prodex_manual_redeem_reset_v1(1,0,0,-1,0,0,0) == -1
    assert prodex_manual_redeem_reset_v1(1,0,0,0,1,0,0) == -1

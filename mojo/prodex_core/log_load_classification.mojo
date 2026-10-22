from rich_types import ProdexRichStringView
from rich_text import rich_view_matches_literal


def log_load_event_is_routine(event: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["profile_inflight_saturated"](event, False)
        or rich_view_matches_literal["runtime_proxy_active_limit_reached"](event, False)
        or rich_view_matches_literal["runtime_proxy_lane_limit_reached"](event, False)
    )

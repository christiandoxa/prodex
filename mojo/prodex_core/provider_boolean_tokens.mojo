# Shared compiled boolean token classifier for provider and Gemini policies.
# No exported ABI: both callers retain distinct versioned public contracts.
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_matches_literal


def provider_boolean_token_class(view: ProdexRichStringView) -> Int64:
    var bounds = rich_trim_bounds(view)
    var trimmed = ProdexRichStringView(
        view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]),
    )
    if (
        rich_view_matches_literal["1"](trimmed, True)
        or rich_view_matches_literal["true"](trimmed, True)
        or rich_view_matches_literal["yes"](trimmed, True)
        or rich_view_matches_literal["on"](trimmed, True)
    ):
        return 1
    if (
        rich_view_matches_literal["0"](trimmed, True)
        or rich_view_matches_literal["false"](trimmed, True)
        or rich_view_matches_literal["no"](trimmed, True)
        or rich_view_matches_literal["off"](trimmed, True)
    ):
        return 0
    return -1

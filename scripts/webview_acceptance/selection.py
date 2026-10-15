"""Admit read-only UIA selection evidence without deriving delivery success.

CompareEndpoints returns signed Int32 ordering, not character distances.
The observations are non-atomic; admission preserves even inconsistent orders
for diagnosis rather than converting them into a gate or repaired selection.
An unavailable pattern, unsupported selection, or absent range is explicit.
"""


FIELDS = {"available", "supported", "range_count", "comparisons"}
ENDPOINTS = {"start_to_document_start", "end_to_document_end",
             "start_to_end", "document_start_to_end"}


def admit(value: dict) -> dict:
    if not isinstance(value, dict) or set(value) != FIELDS:
        raise ValueError("actual UIA selection has an invalid record shape")
    available, supported = value["available"], value["supported"]
    count, comparisons = value["range_count"], value["comparisons"]
    if type(available) is not bool:
        raise ValueError("actual UIA selection availability must be bool")
    if not available:
        if any(item is not None for item in (supported, count, comparisons)):
            raise ValueError("unavailable UIA text pattern carries selection data")
    elif supported not in ("None", "Single", "Multiple"):
        raise ValueError("actual UIA selection support is unknown")
    elif supported == "None":
        if count is not None or comparisons is not None:
            raise ValueError("unsupported UIA selection carries range data")
    else:
        if type(count) is not int or not 0 <= count <= 0x7FFFFFFF:
            raise ValueError("actual UIA selection count exceeds Int32 array length")
        if count == 1:
            if not isinstance(comparisons, dict) or set(comparisons) != ENDPOINTS:
                raise ValueError("unique UIA selection omits endpoint comparisons")
            if any(type(order) is not int or not -0x80000000 <= order <= 0x7FFFFFFF
                   for order in comparisons.values()):
                raise ValueError("actual UIA endpoint comparison exceeds signed Int32")
        elif comparisons is not None:
            raise ValueError("nonunique UIA selection carries unique-range comparisons")
    return value

"""Canonical software visual admission bounds and contextual failures."""

MAX_BYTES, MAX_WIDTH, MAX_HEIGHT = 4 * 1024 * 1024, 800, 600


class VisualError(ValueError):
    """Capture validation failed; report_path identifies collected review evidence."""
    def __init__(self, message, report_path=None):
        super().__init__(message)
        self.report_path = report_path


def require(condition, message):
    if not condition:
        raise VisualError(message)


def dimensions(width, height):
    require(0 < width <= MAX_WIDTH and 0 < height <= MAX_HEIGHT,
             "Image dimensions exceed the 800x600 software budget")

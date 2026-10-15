"""Explicit observed-value representation; OS input retains its original text.

RichEdit's TOM story includes an undeletable final paragraph CR. Its UIA
ValuePattern can expose that complete story while WM_GETTEXT omits the marker.
The caller selects the known control contract before input. No detected-value
fallback, trimming, normalization or extra input converts one representation
into the other. WebView input values retain literal equality.

https://learn.microsoft.com/en-us/windows/win32/api/tom/nn-tom-itextrange
"""

from enum import Enum


class TextValueRepresentation(Enum):
    CONTROL = ""
    RICH_EDIT_STORY = "\r"

    def observed(self, text: str) -> str:
        return text + self.value

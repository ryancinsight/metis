"""Bounded last-operation evidence independent of worker cleanup.

Each complete file replaces its prior value atomically in the owned directory.
Both response and state are admitted before either file changes. Publication of
two files is not a transaction: a native state-write failure can leave new raw
bytes beside the preceding state. Consumers must compare the recorded digest.
No JSON decoding, UIA node synthesis, or acceptance decision occurs here.
"""

import hashlib
import json
from pathlib import Path
import stat
import time

from .contract import MAX_UIA_BYTES
from .retention import publish


def _encoded(value: dict) -> bytes:
    """Admit the complete UTF-8 state under the existing inclusive UIA bound."""
    raw = json.dumps(value, ensure_ascii=False, allow_nan=False,
                     separators=(",", ":")).encode("utf-8")
    if len(raw) > MAX_UIA_BYTES:
        raise ValueError("actual UIA progress state exceeds its byte budget")
    return raw


class Progress:
    """One bounded state and exact latest response for one owned UIA actor."""

    def __init__(self, output: Path):
        try:
            directory = output.resolve(strict=True)
            if not stat.S_ISDIR(directory.stat().st_mode):
                raise NotADirectoryError(str(directory))
        except OSError as error:
            error.add_note(f"resolving actual UIA progress directory {output!r}")
            raise
        self._path = directory / "uia-progress.json"
        self._response = directory / "uia-response.json"
        self._actor_paths = tuple(directory / f"uia-actor-{index}.json" for index in (0, 1))
        self._sequence = 0
        self._state = {"schema": 1, "sequence": "0", "monotonic_ns": "0",
                       "operation": "", "identifiers": [], "request": None,
                       "phase": "", "last_response": None, "details": {}}
        self.mark("initialized")

    @property
    def actor_paths(self) -> tuple[Path, Path]:
        return self._actor_paths

    @property
    def sequence(self) -> int:
        return self._sequence

    def _candidate(self, phase: str, fields: dict, **updates) -> dict:
        if not isinstance(phase, str):
            raise TypeError("actual UIA progress phase must be text")
        return {**self._state, **updates, "phase": phase,
                "monotonic_ns": str(time.monotonic_ns()), "details": fields}

    def _commit(self, value: dict) -> None:
        encoded = _encoded(value)
        committed = json.loads(encoded)
        try:
            publish(self._path, encoded)
        except OSError as error:
            error.add_note(f"publishing actual UIA progress {self._path!r} through "
                           f"{self._path.with_name(self._path.name + '.pending')!r}")
            raise
        self._state = committed

    def operation(self, kind: str, identifiers: tuple[str, ...] = ()) -> None:
        if not isinstance(kind, str) or not isinstance(identifiers, tuple) or any(
                not isinstance(identifier, str) for identifier in identifiers):
            raise TypeError("actual UIA operation requires text and a tuple of text identifiers")
        self._commit(self._candidate("operation", {}, operation=kind,
                                     identifiers=list(identifiers)))

    def input(self, observation: dict) -> None:
        """Keep the last real input boundary independent of later UIA polling.

        The complete state retains the existing inclusive byte bound. Neither
        native facts nor requested text change after a failed publication.
        An observation records separate non-atomic reads; it grants no focus
        or delivery success and does not retry an input operation.
        """
        if not isinstance(observation, dict):
            raise TypeError("actual UIA input observation must be a mapping")
        value = {"monotonic_ns": str(time.monotonic_ns()), "observation": observation}
        self._commit(self._candidate("input_observed", {}, input=value))

    def begin(self, action: str, identifier: str | None) -> int:
        if not isinstance(action, str) or (identifier is not None and not isinstance(identifier, str)):
            raise TypeError("actual UIA request requires a text action and optional text identifier")
        sequence = self._sequence + 1
        self._commit(self._candidate("request_before_send", {}, sequence=str(sequence),
                                     request={"action": action, "id": identifier}))
        self._sequence = sequence
        return sequence

    def received(self, raw: bytes) -> None:
        if not isinstance(raw, bytes):
            raise TypeError("actual UIA response must retain its original bytes")
        if len(raw) > MAX_UIA_BYTES:
            raise ValueError("actual UIA response exceeds its byte budget")
        summary = {"sequence": str(self._sequence), "bytes": len(raw),
                   "sha256": hashlib.sha256(raw).hexdigest()}
        value = self._candidate("response_received", {}, last_response=summary)
        encoded = _encoded(value)
        path = self._response
        try:
            publish(path, raw)
            # The response now exists even if state publication fails.
            self._state = value
            path = self._path
            publish(path, encoded)
        except OSError as error:
            error.add_note(f"publishing actual UIA evidence {path!r} through "
                           f"{path.with_name(path.name + '.pending')!r}")
            raise

    def mark(self, phase: str, *, primary: BaseException | None = None, **fields) -> None:
        try:
            self._commit(self._candidate(phase, fields))
        except Exception as error:
            if primary is None:
                raise
            notes = " ; ".join(getattr(error, "__notes__", ()))
            primary.add_note(f"retaining actual UIA progress {self._path!r}: "
                             f"{type(error).__name__}: {error}; {notes}")

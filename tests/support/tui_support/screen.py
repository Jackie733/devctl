"""Interpret the terminal output needed by the PTY checks."""

import base64
import codecs
import re
import unicodedata


class Screen:
    """Interpret the cursor/erase sequences emitted by the Crossterm backend."""

    def __init__(self, width=120, height=30):
        self.resize(width, height)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.pending = ""
        self.clipboard = None
        self.clipboard_requests = 0

    def resize(self, width, height):
        self.width, self.height = width, height
        self.rows = [[" "] * width for _ in range(height)]
        self.x = self.y = 0

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith("\x1b]"):
                match = re.match(
                    r"\x1b\]52;c;([A-Za-z0-9+/=]*)(?:\x07|\x1b\\)", self.pending
                )
                if not match:
                    assert len(self.pending) < 100000, (
                        "Clipboard command exceeded its budget"
                    )
                    return
                self.clipboard = base64.b64decode(match.group(1), validate=True).decode(
                    "utf-8"
                )
                self.clipboard_requests += 1
                self.pending = self.pending[match.end() :]
                continue
            if self.pending.startswith("\x1b["):
                match = re.match(r"\x1b\[([0-9;?]*)([ -/]*)([@-~])", self.pending)
                if not match:
                    return
                args, _, command = match.groups()
                self.pending = self.pending[match.end() :]
                values = [
                    int(value) if value else 0 for value in args.lstrip("?").split(";")
                ]
                count = values[0] or 1
                if command in ("H", "f"):
                    self.y = max(0, min(self.height - 1, count - 1))
                    self.x = max(
                        0,
                        min(self.width - 1, (values[1] if len(values) > 1 else 1) - 1),
                    )
                elif command == "G":
                    self.x = min(self.width - 1, count - 1)
                elif command == "A":
                    self.y = max(0, self.y - count)
                elif command == "B":
                    self.y = min(self.height - 1, self.y + count)
                elif command == "C":
                    self.x = min(self.width - 1, self.x + count)
                elif command == "D":
                    self.x = max(0, self.x - count)
                elif command == "J" and values[0] in (2, 3):
                    self.rows = [[" "] * self.width for _ in range(self.height)]
                elif command == "K":
                    self.rows[self.y][self.x :] = [" "] * (self.width - self.x)
                continue
            if self.pending[0] == "\x1b":
                if len(self.pending) < 2:
                    return
                raise AssertionError(f"Unexpected escape: {self.pending[:30]!r}")
            character, self.pending = self.pending[0], self.pending[1:]
            if character == "\r":
                self.x = 0
            elif character == "\n":
                self.y = min(self.height - 1, self.y + 1)
            elif character >= " ":
                width = (
                    2 if unicodedata.east_asian_width(character) in ("W", "F") else 1
                )
                if self.x < self.width and self.y < self.height:
                    row = self.rows[self.y]
                    # Overwriting either half of a wide glyph clears its other
                    # half, as a real terminal does. Incremental redraws may
                    # replace Chinese path characters with ordinary spaces.
                    if row[self.x] == "" and self.x > 0:
                        row[self.x - 1] = " "
                    if self.x + 1 < self.width and row[self.x + 1] == "":
                        row[self.x + 1] = " "
                    if width == 2 and self.x + 2 < self.width and row[self.x + 2] == "":
                        row[self.x + 2] = " "
                    row[self.x] = character
                    if width == 2 and self.x + 1 < self.width:
                        row[self.x + 1] = ""
                self.x += width

    def text(self):
        return "\n".join("".join(row) for row in self.rows)

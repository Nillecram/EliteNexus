"""Check a complete Elite Nexus static site build without network access.

Usage: python tools/site/check.py --root target-agents/site-preview
Checks HTML links/assets, fragments, page metadata, moved-page redirects and screenshot links.
Only the Python standard library is required.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
import re
import sys
from urllib.parse import quote, unquote, urljoin, urlsplit


BASE = "https://uruskan.github.io/EliteNexus/"
DEFAULT_ROOT = Path(__file__).resolve().parents[2] / "site"
VOID_ELEMENTS = frozenset(
    "area base br col embed hr img input link meta param source track wbr".split()
)
IMAGE_SUFFIXES = frozenset((".png", ".jpg", ".jpeg", ".webp", ".avif", ".gif", ".svg"))


@dataclass
class Reference:
    tag: str
    attribute: str
    value: str
    line: int


@dataclass
class Screenshot:
    src: str
    alt: str
    anchor: dict[str, str | None] | None
    line: int


class Document(HTMLParser):
    def __init__(self, path: Path, root: Path):
        super().__init__(convert_charrefs=True)
        self.path = path
        self.relative = path.relative_to(root).as_posix()
        self.url = urljoin(BASE, quote(self.relative))
        self.ids: dict[str, int] = {}
        self.duplicates: list[tuple[str, int, int]] = []
        self.references: list[Reference] = []
        self.screenshots: list[Screenshot] = []
        self.stack: list[tuple[str, dict[str, str | None]]] = []
        self.titles: list[list[str]] = []
        self.descriptions: list[str] = []
        self.canonicals: list[str] = []
        self.refresh: str | None = None
        self.noindex = False
        self.h1_count = 0

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        line = self.getpos()[0]
        element_id = attributes.get("id")
        if element_id is not None:
            if element_id in self.ids:
                self.duplicates.append((element_id, self.ids[element_id], line))
            else:
                self.ids[element_id] = line
        if tag == "h1":
            self.h1_count += 1
        if tag == "title":
            self.titles.append([])
        if tag == "meta" and (attributes.get("name") or "").lower() == "description":
            self.descriptions.append(attributes.get("content") or "")
        if tag == "link" and "canonical" in (attributes.get("rel") or "").lower().split():
            self.canonicals.append(attributes.get("href") or "")
        if tag == "meta" and (attributes.get("http-equiv") or "").lower() == "refresh":
            match = re.search(r"url\s*=\s*(\S+)", attributes.get("content") or "", re.I)
            self.refresh = match.group(1).strip("'\"") if match else ""
            self.references.append(Reference(tag, "refresh", self.refresh, line))
        if tag == "meta" and (attributes.get("name") or "").lower() == "robots":
            self.noindex = "noindex" in (attributes.get("content") or "").lower()
        for attribute in ("href", "src"):
            if attribute in attributes:
                self.references.append(Reference(tag, attribute, attributes[attribute] or "", line))
        if tag == "img" and "src" in attributes:
            src = attributes.get("src") or ""
            in_figure = any(
                "screenshot" in (parent.get("class") or "").split()
                for _, parent in self.stack
            )
            if "screenshots" in unquote(urlsplit(src).path).split("/") or in_figure:
                anchor = next((a for name, a in reversed(self.stack) if name == "a"), None)
                self.screenshots.append(Screenshot(src, attributes.get("alt") or "", anchor, line))
        if tag not in VOID_ELEMENTS:
            self.stack.append((tag, attributes))

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID_ELEMENTS:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, -1, -1):
            if self.stack[index][0] == tag:
                del self.stack[index:]
                break

    def handle_data(self, data):
        if self.titles and any(tag == "title" for tag, _ in self.stack):
            self.titles[-1].append(data)


def local_target(root: Path, page_url: str, value: str):
    """Return (path, fragment), None for external URLs, or raise for bad locals."""
    if "\\" in value:
        raise ValueError("URL contains a backslash instead of a forward slash")
    url = urlsplit(urljoin(page_url, value))
    production = urlsplit(BASE)
    if url.scheme not in ("http", "https") or url.netloc != production.netloc:
        return None
    prefix = production.path
    path = unquote(url.path)
    if path == prefix.rstrip("/"):
        relative = ""
    elif path.startswith(prefix):
        relative = path[len(prefix):]
    else:
        raise ValueError("local URL is outside the /EliteNexus/ project path")
    target = (root / relative).resolve()
    if not target.is_relative_to(root):
        raise ValueError("local URL escapes the site output directory")
    if target.is_dir() or path.endswith("/"):
        target /= "index.html"
    return target, unquote(url.fragment)


def check(root: Path) -> int:
    root = root.resolve()
    if not root.is_dir():
        print(f"ERROR: site root is not a directory: {root}", file=sys.stderr)
        return 1
    files = sorted(p for p in root.rglob("*") if p.is_file() and p.suffix.lower() in (".html", ".htm"))
    if not files:
        print(f"ERROR: no HTML pages found in {root}", file=sys.stderr)
        return 1
    errors: list[str] = []
    documents: dict[Path, Document] = {}

    def fail(document, message, line=None):
        location = document.relative + (f":{line}" if line is not None else "")
        errors.append(f"{location}: {message}")

    for path in files:
        document = Document(path, root)
        try:
            document.feed(path.read_text(encoding="utf-8"))
            document.close()
        except (OSError, UnicodeError, ValueError) as error:
            fail(document, f"could not read HTML: {error}")
            continue
        documents[path.resolve()] = document

    for document in documents.values():
        for identifier, original_line, line in document.duplicates:
            fail(document, f"duplicate id {identifier!r} (first used on line {original_line})", line)
        if document.h1_count != 1:
            fail(document, f"expected one h1, found {document.h1_count}")
        if len(document.titles) != 1 or not "".join(document.titles[0]).strip():
            fail(document, "expected one nonempty title")
        if len(document.descriptions) != 1 or not document.descriptions[0].strip():
            fail(document, "expected one nonempty meta description")
        if document.refresh is not None:
            # A page that moved: it must point search engines at its destination and ask not to be indexed.
            if not document.refresh:
                fail(document, "redirect page has a refresh without a destination")
            if not document.noindex:
                fail(document, "redirect page must carry <meta name=\"robots\" content=\"noindex\">")
            canonical_path = unquote(urlsplit(urljoin(document.url, document.refresh)).path)
            expected_canonical = urljoin(BASE, quote(canonical_path.removeprefix(urlsplit(BASE).path)))
        else:
            canonical_path = document.relative
            expected_canonical = urljoin(BASE, quote(canonical_path))
        if expected_canonical.endswith("index.html"):
            expected_canonical = expected_canonical[:-len("index.html")]
        if document.canonicals != [expected_canonical]:
            fail(document, f"canonical must be {expected_canonical!r}; found {document.canonicals!r}")

        for reference in document.references:
            if not reference.value.strip() and reference.attribute == "src":
                fail(document, f"empty {reference.tag} src", reference.line)
                continue
            try:
                resolved = local_target(root, document.url, reference.value)
            except ValueError as error:
                fail(document, f"{reference.attribute}={reference.value!r}: {error}", reference.line)
                continue
            if resolved is None:
                continue
            target, fragment = resolved
            if not target.is_file():
                fail(document, f"missing local target for {reference.attribute}={reference.value!r}", reference.line)
                continue
            if fragment and target.suffix.lower() in (".html", ".htm"):
                destination = documents.get(target.resolve())
                if destination is None or fragment not in destination.ids:
                    fail(document, f"missing fragment #{fragment} in {target.relative_to(root).as_posix()}", reference.line)

        for screenshot in document.screenshots:
            alt = screenshot.alt.strip()
            if len(alt) < 20 or len(re.findall(r"\w+", alt)) < 4 or alt.lower().endswith(tuple(IMAGE_SUFFIXES)):
                fail(document, "screenshot needs descriptive alt text (describe what the image shows)", screenshot.line)
            # Every screenshot is a plain link to its own full-size file: no viewer script, no dead ends.
            if screenshot.anchor is None:
                fail(document, "screenshot must be inside a link to its full-size image", screenshot.line)
                continue
            href = screenshot.anchor.get("href") or ""
            try:
                image = local_target(root, document.url, screenshot.src)
                original = local_target(root, document.url, href)
            except ValueError:
                # The normal reference checks above already explain malformed URLs.
                continue
            if not href or original is None or not original[0].is_file() or original[0].suffix.lower() not in IMAGE_SUFFIXES:
                fail(document, "screenshot must link directly to a local image file", screenshot.line)
            elif image is None or original[0] != image[0]:
                fail(document, "screenshot must link to its own full-size image file", screenshot.line)

    page_count = len(documents)
    reference_count = sum(len(document.references) for document in documents.values())
    screenshot_count = sum(len(document.screenshots) for document in documents.values())
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        print(f"FAILED: {len(errors)} issue(s) across {page_count} pages.", file=sys.stderr)
        return 1
    print(f"PASS: {page_count} pages, {reference_count} URL references, {screenshot_count} screenshots linked to their full-size files.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT, help="Complete site output directory (default: repository site/)")
    return check(parser.parse_args().root)


if __name__ == "__main__":
    raise SystemExit(main())

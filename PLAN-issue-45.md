# Plan: Python Bindings for hallucinator-rs (Issue #45)

## Goal

Expose the Rust PDF-parsing and reference-validation pipeline to Python via PyO3,
so users can **iterate on regexes and parsing behavior** from Python without
rebuilding the Rust binary. Secondary goal: let Python scripts drive the full
validation pipeline programmatically.

---

## Key Use Case

A researcher gets bad parses on their paper. Today they'd have to:
1. Clone the repo, install Rust toolchain, edit hardcoded regexes in `.rs` files, rebuild.

With bindings they could instead:
```python
from hallucinator import PdfExtractor, Config

ext = PdfExtractor()
ext.set_section_header_regex(r"(?i)\n\s*(?:References|Bibliograf[ií]a)\s*\n")
ext.set_segmentation_regex(r"\n\s*\[(\d+)\]\s*")
refs = ext.extract("paper.pdf")

for r in refs:
    print(r.title, r.authors)
```

---

## Tooling Choice: PyO3 + Maturin

| Option | Pros | Cons |
|--------|------|------|
| **PyO3 + maturin** | First-class Python types, pip-installable wheels, async support via `pyo3-asyncio`, most popular | Tight coupling to CPython ABI |
| C FFI + cffi/ctypes | Language-agnostic | Manual memory management, no rich types, painful strings |
| uniffi (Mozilla) | Multi-language from one IDL | Less mature Python story, extra build step |

**Recommendation: PyO3 + maturin.** It's the standard for Rust→Python and gives us
native Python classes, exceptions, iterators, and `pip install` distribution.

---

## Architecture

```
hallucinator-rs/
├── crates/
│   ├── hallucinator-pdf/       # Already a lib crate (no changes needed)
│   ├── hallucinator-core/      # Already a lib crate (no changes needed)
│   ├── hallucinator-dblp/      # Already a lib crate (no changes needed)
│   ├── hallucinator-acl/       # Already a lib crate (no changes needed)
│   ├── hallucinator-bbl/       # Already a lib crate (no changes needed)
│   └── ...
│
├── crates/hallucinator-python/ # NEW — thin PyO3 wrapper crate
│   ├── Cargo.toml              # cdylib, depends on -pdf, -core, -dblp, -acl, -bbl
│   ├── src/
│   │   ├── lib.rs              # #[pymodule] root
│   │   ├── extractor.rs        # PyPdfExtractor (wraps hallucinator-pdf)
│   │   ├── validator.rs        # PyValidator (wraps hallucinator-core)
│   │   ├── config.rs           # PyConfig, PyRegexOverrides
│   │   ├── types.rs            # PyReference, PyValidationResult, etc.
│   │   └── errors.rs           # Python exception mapping
│   └── hallucinator.pyi        # Type stubs for IDE support
│
└── pyproject.toml              # NEW (in hallucinator-rs/, maturin build config)
```

The existing library crates stay untouched as pure Rust. The new `hallucinator-python`
crate is a thin wrapper that:
1. Re-exports Rust types as Python classes
2. Adds regex-override configuration plumbing
3. Bridges async (tokio) to sync Python calls (with optional async via `pyo3-asyncio`)

---

## Phased Implementation

### Phase 1 — Regex-configurable PDF extraction (core use case)

This is the highest-value phase: let users override the ~100 hardcoded regex
patterns from Python.

**Step 1.1: Add `RegexOverrides` to `hallucinator-pdf`**

Currently all regexes are `static Lazy<Regex>`. Refactor so each parsing module
accepts an optional overrides struct:

```rust
// hallucinator-pdf/src/config.rs (new file)
pub struct PdfParsingConfig {
    // Section detection
    pub section_header_re: Option<String>,
    pub section_end_markers_re: Option<String>,
    pub fallback_fraction: Option<f64>,      // default 0.7

    // Segmentation
    pub ieee_segment_re: Option<String>,     // [1] style
    pub numbered_segment_re: Option<String>, // 1. style
    pub aaai_segment_re: Option<String>,
    pub springer_segment_re: Option<String>,
    pub fallback_segment_re: Option<String>,

    // Title extraction
    pub quote_patterns: Option<Vec<String>>,
    pub venue_cutoff_patterns: Option<Vec<String>>,
    pub subtitle_patterns: Option<Vec<String>>,

    // Author extraction
    pub em_dash_re: Option<String>,
    pub author_connector_re: Option<String>,
    pub max_authors: Option<usize>,

    // Identifiers
    pub doi_re: Option<String>,
    pub arxiv_re: Option<String>,

    // Text processing
    pub compound_suffixes: Option<Vec<String>>,
    pub ligature_map: Option<Vec<(String, String)>>,

    // Filtering
    pub min_title_words: Option<usize>,      // default 4
    pub non_academic_url_re: Option<String>,
    pub academic_url_re: Option<String>,
}

impl Default for PdfParsingConfig { /* all None = use built-in defaults */ }
```

Then thread this config through the existing functions, falling back to the
current hardcoded `Lazy<Regex>` when `None`.

**Step 1.2: Create `hallucinator-python` crate**

Thin PyO3 wrapper:

```python
# Python API
from hallucinator import PdfExtractor, Reference

ext = PdfExtractor()

# Override any regex
ext.section_header_regex = r"(?i)\n\s*(?:References|Bibliograf[ií]a)\s*\n"
ext.ieee_segment_regex = r"\n\s*\[(\d+)\]\s*"
ext.min_title_words = 3
ext.add_venue_cutoff_pattern(r"(?i)\.\s*Nature\b.*$")
ext.add_compound_suffix("aware")

# Extract
refs: list[Reference] = ext.extract("paper.pdf")

# Inspect
for r in refs:
    print(r.title, r.authors, r.doi, r.arxiv_id, r.raw_citation)

# Also expose lower-level steps
text = ext.extract_text("paper.pdf")           # raw text
section = ext.find_references_section(text)     # just the ref section
segments = ext.segment_references(section)      # individual ref strings
refs = ext.parse_references(segments)           # structured References
```

Key design points:
- Each step is independently callable (extract_text → find_section → segment → parse)
- Regex overrides apply at each step
- Returns plain Python objects (dataclasses-style), not opaque handles

**Step 1.3: Maturin build + `pip install`**

```toml
# hallucinator-rs/pyproject.toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "hallucinator"
requires-python = ">=3.9"

[tool.maturin]
features = ["pyo3/extension-module"]
module-name = "hallucinator._hallucinator"
```

Users install with: `pip install .` (from `hallucinator-rs/`) or eventually
from PyPI.

---

### Phase 2 — Validation pipeline

Expose the full check-references flow:

```python
from hallucinator import PdfExtractor, Validator, Config

config = Config()
config.s2_api_key = "..."
config.openalex_key = "..."
config.max_concurrent_refs = 4
config.db_timeout_secs = 10
config.disabled_dbs = ["openalex"]

ext = PdfExtractor()
refs = ext.extract("paper.pdf")

validator = Validator(config)
results = validator.check(refs)  # blocks, runs tokio runtime internally

for r in results:
    print(r.title, r.status, r.source, r.retraction_info)
    for db in r.db_results:
        print(f"  {db.name}: {db.status} ({db.elapsed_ms}ms)")
```

This phase requires bridging tokio async → sync Python. PyO3 handles this
via `pyo3_async_runtimes::tokio` or by just calling `Runtime::block_on()` inside
the Python-facing function.

**Progress callbacks:**

```python
def on_progress(event):
    print(f"[{event.type}] {event.ref_title}: {event.message}")

results = validator.check(refs, progress=on_progress)
```

---

### Phase 3 — Offline databases

```python
from hallucinator import DblpDatabase, AclDatabase

# Build/update
DblpDatabase.build("dblp.db", progress=print)

# Query
db = DblpDatabase("dblp.db")
result = db.query("Attention Is All You Need")
print(result.title, result.authors, result.url)
print(db.staleness_days)
```

---

### Phase 4 — BibTeX parsing + async Python API

```python
from hallucinator import BblParser

refs = BblParser.parse("refs.bbl")

# Async API (optional, phase 4)
import asyncio
from hallucinator import AsyncValidator

async def main():
    validator = AsyncValidator(config)
    async for event in validator.check_stream(refs):
        print(event)

asyncio.run(main())
```

---

## What Changes in Existing Crates

The existing crates need **one structural change**: the `hallucinator-pdf` crate's
parsing functions need to accept an optional `PdfParsingConfig` parameter instead
of only using hardcoded `static Lazy<Regex>` values. This is a backward-compatible
change — all current call sites pass `Default::default()` (or `None`) and get
the existing behavior.

No other crates need modification for Phase 1. The `-core`, `-dblp`, `-acl`, and
`-bbl` crates already have clean public APIs that the Python wrapper can call directly.

---

## Estimated Scope

| Phase | New/Changed Files | Complexity |
|-------|-------------------|------------|
| Phase 1 (PDF + regexes) | ~8 new files, ~5 modified | Medium-high (regex threading is mechanical but touches many functions) |
| Phase 2 (Validation) | ~3 new files | Medium (async bridging) |
| Phase 3 (Offline DBs) | ~2 new files | Low (clean existing APIs) |
| Phase 4 (BBL + async) | ~2 new files | Low-medium |

---

## Open Questions

1. **Package name on PyPI** — `hallucinator`? `hallucinator-rs`? `hallref`?
2. **Minimum Python version** — 3.9+ seems reasonable (matches PyO3 support)
3. **Should regex overrides also be loadable from a TOML/JSON config file?** This
   would let users share "fix packs" for specific paper formats without writing Python.
4. **Should we expose a way to add entirely new segmentation strategies from Python?**
   Currently there are 5 hardcoded strategies tried in order. A plugin system would
   be more powerful but significantly more complex.
5. **Wheel distribution** — Build for manylinux, macOS (arm64+x86), Windows? Or
   source-only initially?

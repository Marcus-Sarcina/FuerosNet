"""Shared helpers for the acceptance catalogue tools.

The catalogue is `../acceptance.json`.  Every id is `XXX-NN`; the prefix fixes
the area, and the area fixes the gap row in `Robot/implementation-plan.md`
section 8.2 that the entry helps cover.  No exemptions anywhere (CLAUDE.md).
"""
import json, os, re

HERE = os.path.dirname(os.path.abspath(__file__))
ACCEPTANCE = os.path.dirname(HERE)
WORKSPACE = os.path.dirname(ACCEPTANCE)
ROOT = os.path.dirname(WORKSPACE)
CATALOGUE = os.path.join(ACCEPTANCE, "acceptance.json")
TESTS_DIR = os.path.join(ACCEPTANCE, "tests")

DOCS = {"design": "network-design.md", "wire-format.md": "wire-format.md",
        "light-client-requirements.md": "light-client-requirements.md",
        "infra-client-requirements.md": "infra-client-requirements.md",
        "resource-requirements.md": "resource-requirements.md"}

# prefix -> (area, gap).  The gap keys are the rows of the plan's section 8.2.
PREFIX = {"DEC": "decoder", "TRN": "transport", "SES": "session", "QUE": "queue",
          "ARC": "archive", "TOP": "topology", "RES": "resolution",
          "REP": "replication-peering", "PRP": "propagation", "CUR": "currency",
          "MET": "metric", "CER": "ceremony", "REC": "recovery", "PAY": "payload",
          "RSC": "resources", "PRD": "product", "TRV": "traversal",
          "DMN": "daemon", "SUB": "submission", "PRT": "participant"}
GAPS = list(dict.fromkeys(PREFIX.values()))
MILESTONES = {1, 2, 3, 4, 5, "after-5", "manual"}
KINDS = {"positive", "negative", "must-accept", "liveness", "robustness", "manual", "withdrawn"}
ORACLES = {"fixture", "model", "behaviour"}
FIELDS = ["id", "area", "gap", "title", "milestone", "kind", "spec", "rule",
          "given", "when", "then", "oracle", "interpretation"]
# Optional, and only on an entry holding a transaction rule
# (`Robot/transaction-rules.md`).  `holds` is a list of [type, condition]
# pairs, or [type, condition, side] where the side differs from the entry's
# own kind — one test may hold the positive of one condition and the
# negative of another, and a rule whose enforcement *is* its observation
# would otherwise need an entry split in two to say nothing new.  Written
# out rather than crossed from two lists, because an entry may check a rule
# that binds on one type and not another and a cross product would claim
# coverage nobody wrote.  `deferred` is why it cannot run yet.
OPTIONAL = ["holds", "deferred"]
# The six the wire defines, and 0 for a condition that binds whatever the
# type — the envelope's shape, the COSE profile, the storage rule. Those
# are checked before the type is looked at, so pairing them per type would
# manufacture six demands where the rule makes one.
TRANSACTIONS = {0, 1, 2, 3, 4, 5, 7}

ID_RE = re.compile(r"^([A-Z]{3})-(\d{2})$")
CITE_RE = re.compile(r"^(design|wire-format\.md|light-client-requirements\.md|"
                     r"infra-client-requirements\.md|resource-requirements\.md) "
                     r"(?:§([0-9]+(?:\.[0-9]+)*)|Appendix ([A-Z](?:\.[0-9]+)?))$")
HEADING_RE = re.compile(r"^(#{1,6})\s+(?:Appendix\s+)?([0-9A-Z][0-9A-Za-z.]*)[\s.]")
MARKER_RE = re.compile(r"acceptance:\s*([A-Z]{3}-\d{2})")
CONDITION_RE = re.compile(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$")


def load():
    with open(CATALOGUE, encoding="utf-8") as f:
        return json.load(f)


def sections(doc):
    """{number: (depth, start_line, end_line, text)} for one specification file."""
    lines = open(os.path.join(ROOT, DOCS[doc]), encoding="utf-8").read().split("\n")
    heads = []
    for i, line in enumerate(lines):
        m = HEADING_RE.match(line)
        if m:
            heads.append((len(m.group(1)), m.group(2).rstrip("."), i))
    out = {}
    for k, (depth, num, start) in enumerate(heads):
        end = len(lines)
        for d2, _, s2 in heads[k + 1:]:
            if d2 <= depth:
                end = s2
                break
        out[num] = (depth, start, end, "\n".join(lines[start:end]))
    return out


def normalise(text):
    """Markdown emphasis, code marks and typographic quotes removed, whitespace collapsed."""
    text = text.replace("**", "").replace("`", "").replace("*", "")
    text = (text.replace("“", '"').replace("”", '"')
                .replace("‘", "'").replace("’", "'")
                .replace("—", "-").replace("–", "-").replace("‑", "-")
                .replace("−", "-").replace("‐", "-").replace("‒", "-")
                .replace("\u00a0", " "))
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)   # links keep their text
    return re.sub(r"\s+", " ", text).strip().lower()


def cite_target(cite):
    """('design', '7.5.2') or ('design', 'A.3') from a qualified citation, else None."""
    m = CITE_RE.match(cite)
    if not m:
        return None
    return m.group(1), (m.group(2) or m.group(3))


def implemented_ids():
    """ids carrying an `acceptance: XXX-NN` marker anywhere in the workspace's
    Rust sources, outside the generated stubs and build output."""
    found = {}
    for dirpath, dirnames, filenames in os.walk(WORKSPACE):
        dirnames[:] = [d for d in dirnames if d != "target"]
        if os.path.abspath(dirpath).startswith(os.path.abspath(TESTS_DIR)):
            continue
        for fn in filenames:
            if fn.endswith(".rs"):
                p = os.path.join(dirpath, fn)
                for m in MARKER_RE.finditer(open(p, encoding="utf-8").read()):
                    found.setdefault(m.group(1), os.path.relpath(p, WORKSPACE))
    return found

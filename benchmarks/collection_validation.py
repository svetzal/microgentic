"""Read-only collection checks against a registered Git baseline.

Coordinators call ``verify_original_tests`` after collecting the full Git
workspace and verifying its binary patch, and before accepting a result or
dispatching another trial. The baseline is the registered task commit, never a
revision the agent chose. The collected revision defaults to ``HEAD``, the
committed workspace used for patch verification. Working-tree inspection and
patch checks remain separate requirements.

The check compares the Git mode, type and object ID of every ``tests/`` entry
in the baseline. Added tests pass. Changed content, deletion, rename, an
executable-bit change or a symlink in place of an original fails. Invalid
revisions and Git failures fail closed. Run the regression suite with::

    python3 -m unittest discover -s benchmarks -p 'test_collection_validation.py' -v
"""

from pathlib import Path
import subprocess


class OriginalTestsChanged(ValueError):
    """The collected revision changed a test present in the baseline."""


def _test_entries(workspace: Path, revision: str) -> dict[bytes, bytes]:
    # NUL delimiters preserve filenames containing tabs or newlines. Comparing
    # mode/type/object protects content, executable bits and symlink targets.
    tree = subprocess.check_output(
        ["git", "ls-tree", "-r", "-z", revision, "--", "tests/"],
        cwd=workspace,
        stderr=subprocess.PIPE,
        timeout=30,
    )
    return {
        path: metadata
        for entry in tree.split(b"\0")
        if entry
        for metadata, path in [entry.split(b"\t", 1)]
    }


def verify_original_tests(
    workspace: Path, baseline_revision: str, collected_revision: str = "HEAD"
) -> None:
    """Allow new tests; require every baseline test's Git entry to be intact.

    Validate the committed collection revision, as used for the binary patch,
    rather than transient working-tree files. Invalid Git revisions fail closed.
    This does not change or accept any preserved experiment result.
    """
    baseline = _test_entries(workspace, baseline_revision)
    collected = _test_entries(workspace, collected_revision)
    changed = sorted(path for path, entry in baseline.items() if collected.get(path) != entry)
    if changed:
        names = ", ".join(repr(path.decode("utf-8", errors="backslashreplace")) for path in changed)
        raise OriginalTestsChanged(f"Original tests altered: {names}")

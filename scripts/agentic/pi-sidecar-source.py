#!/usr/bin/env python3
"""Verify a materialized Pi snapshot, including exact internal Git symlinks."""

import os
import pathlib
import stat
import subprocess
import sys


def verify(cache: str, ref: str, source: str) -> None:
    root = pathlib.Path(source)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("sidecar source root is unsafe")
    root = root.resolve(strict=True)
    entries = {}
    tree = subprocess.check_output(["git", "--git-dir=" + cache, "ls-tree", "-rz", ref])
    for record in tree.split(b"\0"):
        if not record:
            continue
        metadata, raw_path = record.split(b"\t", 1)
        mode, kind, oid = metadata.split()
        if kind != b"blob" or mode not in (b"100644", b"100755", b"120000"):
            raise ValueError("sidecar source contains an unqualified object/submodule")
        child = os.fsdecode(raw_path)
        if pathlib.PurePosixPath(child).is_absolute() or ".." in pathlib.PurePosixPath(child).parts:
            raise ValueError("sidecar source path escapes snapshot")
        entries[child] = (mode, oid)

    actual = set()
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in directories:
            if pathlib.Path(parent, name).is_symlink():
                raise ValueError("sidecar source directory symlink")
        for name in files:
            path = pathlib.Path(parent, name)
            child = path.relative_to(root).as_posix()
            actual.add(child)
            if child not in entries:
                raise ValueError("sidecar source has an untracked file: " + child)
            mode, _ = entries[child]
            observed = path.lstat().st_mode
            if mode == b"120000":
                if not stat.S_ISLNK(observed):
                    raise ValueError("sidecar pinned link was replaced: " + child)
                target = os.readlink(path)
                if not target or os.path.isabs(target):
                    raise ValueError("sidecar source link must be relative: " + child)
                resolved = path.resolve(strict=True)
                try:
                    destination = resolved.relative_to(root).as_posix()
                except ValueError:
                    raise ValueError("sidecar source link escapes snapshot: " + child) from None
                if destination not in entries or entries[destination][0] == b"120000" or not resolved.is_file():
                    raise ValueError("sidecar source link target is unqualified: " + child)
            elif not stat.S_ISREG(observed):
                raise ValueError("sidecar source file was replaced: " + child)
    if actual != set(entries):
        raise ValueError("sidecar source file inventory differs")

    # The pinned tree has thousands of files. One Git batch avoids spawning a
    # process per blob while still comparing every byte against its exact OID.
    with subprocess.Popen(
        ["git", "--git-dir=" + cache, "cat-file", "--batch"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
    ) as blobs:
        try:
            for child, (mode, oid) in entries.items():
                blobs.stdin.write(oid + b"\n")
                blobs.stdin.flush()
                header = blobs.stdout.readline().split()
                if len(header) != 3 or header[0] != oid or header[1] != b"blob":
                    raise ValueError("sidecar pinned blob is unavailable: " + child)
                size = int(header[2])
                content = blobs.stdout.read(size)
                if len(content) != size or blobs.stdout.read(1) != b"\n":
                    raise ValueError("sidecar pinned blob is truncated: " + child)
                path = root / child
                if mode == b"120000":
                    observed = os.fsencode(os.readlink(path))
                else:
                    if child == "Cargo.toml" and b"[workspace]" not in content:
                        content += b"\n[workspace]\n"
                    observed = path.read_bytes()
                if observed != content:
                    raise ValueError("sidecar pinned source content differs: " + child)
        finally:
            blobs.stdin.close()
        if blobs.wait() != 0:
            raise ValueError("sidecar pinned blob reader failed")


if __name__ == "__main__":
    try:
        verify(*sys.argv[1:])
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from None

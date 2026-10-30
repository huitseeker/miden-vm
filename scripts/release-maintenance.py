#!/usr/bin/env python3
"""Build release-maintenance PRs from validated issue-to-PR mappings."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

REPOSITORY = "0xMiden/miden-vm"
TAG = re.compile(r"v(0\.36\.(0|[1-9][0-9]*))")
ISSUE = re.compile(r"https://github\.com/0xMiden/miden-vm/issues/([1-9][0-9]*)", re.I)
PULL = re.compile(r"https://github\.com/0xMiden/miden-vm/pull/([1-9][0-9]*)", re.I)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def run(repo: Path, *args: str) -> str:
    return subprocess.run(
        args, cwd=repo, check=True, text=True, capture_output=True
    ).stdout


def git(repo: Path, *args: str) -> str:
    return run(repo, "git", *args).strip()


def github(repo: Path, *args: str) -> object:
    return json.loads(run(repo, "gh", *args))


def ancestor(repo: Path, older: str, newer: str) -> bool:
    return (
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", older, newer],
            cwd=repo,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        == 0
    )


def read_at(repo: Path, ref: str, path: str) -> str:
    return run(repo, "git", "show", f"{ref}:{path}")


def sections(contents: str) -> tuple[str, list[tuple[str, str]]]:
    headings = list(re.finditer(r"^## (.+)$", contents, re.M))
    require(bool(headings), "changelog has no release headings")
    blocks = []
    for index, heading in enumerate(headings):
        end = (
            headings[index + 1].start() if index + 1 < len(headings) else len(contents)
        )
        blocks.append((heading[1].split(" (")[0], contents[heading.start() : end]))
    require(
        len({key for key, _ in blocks}) == len(blocks),
        "duplicate changelog release heading",
    )
    return contents[: headings[0].start()], blocks


def release_section(contents: str, version: str) -> str:
    matches = [block for key, block in sections(contents)[1] if key == f"v{version}"]
    require(len(matches) == 1, "released changelog section is missing")
    return matches[0]


def inspect_release(repo: Path, release_id: str, kind: str) -> dict:
    require(re.fullmatch(r"[1-9][0-9]*", release_id) is not None, "invalid release ID")
    release = github(repo, "api", f"repos/{REPOSITORY}/releases/{release_id}")
    match = TAG.fullmatch(release["tag_name"])
    require(
        match is not None and not release["draft"] and not release["prerelease"],
        "expected a published stable v0.36.x release",
    )
    published = dt.datetime.fromisoformat(
        release["published_at"].replace("Z", "+00:00")
    )
    require(published.utcoffset() == dt.timedelta(0), "release timestamp must be UTC")
    version = match[1]
    upcoming = f"0.36.{int(match[2]) + 1}"
    base = "main" if kind == "prepare" else "next"
    branch = (
        f"chore/prepare-v{upcoming}"
        if kind == "prepare"
        else f"backmerge/main-v{version}"
    )
    context = {
        "release_id": release_id,
        "release_tag": release["tag_name"],
        "released_version": version,
        "next_version": upcoming,
        "release_date": published.date().isoformat(),
        "kind": kind,
        "base": base,
        "branch": branch,
        "title": (
            f"chore: Prepare v{upcoming}"
            if kind == "prepare"
            else f"chore: Backmerge v{version} into next"
        ),
    }
    # Completed runs return before branch-state checks.
    existing = github(
        repo,
        "pr",
        "list",
        "--repo",
        REPOSITORY,
        "--state",
        "all",
        "--base",
        base,
        "--head",
        branch,
        "--json",
        "number,url,headRepositoryOwner",
        "--limit",
        "100",
    )
    existing = [
        pull
        for pull in existing
        if pull["headRepositoryOwner"]["login"].lower() == "0xmiden"
    ]
    context["duplicate_prs"] = existing
    if existing:
        return context
    require(
        not git(repo, "ls-remote", "--heads", "origin", f"refs/heads/{branch}"),
        "maintenance branch exists without a PR; recover it manually",
    )
    main_sha = git(repo, "rev-parse", "origin/main^{commit}")
    base_sha = git(repo, "rev-parse", f"origin/{base}^{{commit}}")
    tag_sha = git(repo, "rev-parse", f"refs/tags/{release['tag_name']}^{{commit}}")
    require(ancestor(repo, tag_sha, main_sha), "release tag is not on main")
    tagged = tomllib.loads(read_at(repo, tag_sha, "miden-vm/Cargo.toml"))
    require(tagged["package"]["version"] == version, "release tag/version mismatch")
    workspace = tomllib.loads(read_at(repo, main_sha, "Cargo.toml"))
    require(
        workspace["workspace"]["package"]["version"] == version,
        "main is no longer at the released version",
    )
    changelog = read_at(repo, main_sha, "CHANGELOG.md")
    released = release_section(changelog, version)
    # Titles are data; only maintainer-merged PRs included in the tag are evidence.
    query = 'query{repository(owner:"0xMiden",name:"miden-vm"){pullRequests(first:100,states:MERGED,baseRefName:"main",orderBy:{field:UPDATED_AT,direction:DESC}){nodes{number title mergeCommit{oid} closingIssuesReferences(first:100){nodes{number}}}}}}'
    response = github(repo, "api", "graphql", "-f", f"query={query}")
    evidence = {}
    for pull in response["data"]["repository"]["pullRequests"]["nodes"]:
        oid = (pull["mergeCommit"] or {}).get("oid")
        if oid and ancestor(repo, oid, tag_sha):
            evidence[str(pull["number"])] = {
                "number": pull["number"],
                "title": pull["title"],
                "issues": [
                    item["number"] for item in pull["closingIssuesReferences"]["nodes"]
                ],
            }
    for number in set(PULL.findall(released)) - evidence.keys():
        pull = github(repo, "api", f"repos/{REPOSITORY}/pulls/{number}")
        require(
            pull["merged_at"]
            and pull["base"]["ref"] == "main"
            and ancestor(repo, pull["merge_commit_sha"], tag_sha),
            f"PR #{number} was not merged into this release",
        )
        evidence[number] = {"number": int(number), "title": pull["title"], "issues": []}
    context.update(
        main_sha=main_sha,
        base_sha=base_sha,
        changelog=changelog,
        evidence=evidence,
        issues=sorted(set(ISSUE.findall(released)), key=int),
    )
    return context


def clean_changelog(contents: str) -> str:
    # Delete only empty headers; preserve all other bytes.
    headings = list(re.finditer(r"^#{1,4} .+$", contents, re.M))
    for index in range(len(headings) - 1, -1, -1):
        heading = headings[index]
        end = (
            headings[index + 1].start() if index + 1 < len(headings) else len(contents)
        )
        if heading[0].startswith("#### ") and not re.search(
            r"^- ", contents[heading.end() : end], re.M
        ):
            contents = contents[: heading.start()] + contents[heading.end() + 1 :]
    return contents


def dated_changelog(contents: str, context: dict, links: dict[str, int]) -> str:
    original = release_section(contents, context["released_version"])
    updated = re.sub(
        r"^## .+$",
        f"## v{context['released_version']} ({context['release_date']})",
        original,
        count=1,
        flags=re.M,
    )
    for issue, pull in links.items():
        updated = re.sub(
            rf"\[#?{issue}\]\(https://github\.com/0xMiden/miden-vm/issues/{issue}\)",
            f"[#{pull}](https://github.com/{REPOSITORY}/pull/{pull})",
            updated,
            flags=re.I,
        )
        updated = re.sub(
            rf"https://github\.com/0xMiden/miden-vm/issues/{issue}(?![0-9])",
            f"https://github.com/{REPOSITORY}/pull/{pull}",
            updated,
            flags=re.I,
        )
    require(not ISSUE.search(updated), "unresolved issue link")
    entries = re.findall(r"^- .+(?:\n(?!- |#{1,4} ).*)*", updated, re.M)
    require(bool(entries), "release section has no entries")
    for entry in entries:
        numbers = PULL.findall(entry)
        require(
            bool(numbers) and all(number in context["evidence"] for number in numbers),
            "each release entry must link to a verified released PR",
        )
    return clean_changelog(contents.replace(original, updated, 1))


def merge_changelogs(next_contents: str, main_contents: str) -> str:
    preamble, next_blocks = sections(next_contents)
    _, main_blocks = sections(main_contents)
    blocks = list(next_blocks)
    known = dict(blocks)
    for key, block in main_blocks:
        if key in known:
            require(
                known[key].rstrip() == block.rstrip(),
                f"different notes for {key}; resolve this backmerge manually",
            )
            continue
        match = re.match(r"v([0-9]+)\.([0-9]+)\.([0-9]+)", key)
        require(match is not None, f"unsupported main changelog heading {key}")
        version = tuple(map(int, match.groups()))
        position = len(blocks)
        for index, (other, _) in enumerate(blocks):
            parsed = re.match(r"v([0-9]+)\.([0-9]+)\.([0-9]+)", other)
            if parsed and tuple(map(int, parsed.groups())) < version:
                position = index
                break
        blocks.insert(position, (key, block))
        known[key] = block
    return preamble + "".join(block for _, block in blocks)


def manifest_with_version(contents: str, old: str, new: str) -> str:
    section = ""
    output = []
    for line in contents.splitlines(keepends=True):
        header = re.match(r"^\s*\[([^][]+)]\s*(?:#.*)?$", line)
        if header:
            section = header[1].strip()
        if section in {"package", "workspace.package"}:
            line = re.sub(
                rf'^(\s*version\s*=\s*)"{re.escape(old)}"', rf'\g<1>"{new}"', line
            )
        output.append(line)
    return "".join(output)


def lockfile_with_versions(contents: str, old: str, new: str) -> str:
    blocks = re.split(r"(?=^\[\[package]]\s*$)", contents, flags=re.M)
    for index, block in enumerate(blocks):
        if block.startswith("[[package]]") and not re.search(
            r"^source\s*=", block, re.M
        ):
            blocks[index] = re.sub(
                rf'^version = "{re.escape(old)}"$',
                f'version = "{new}"',
                block,
                count=1,
                flags=re.M,
            )
    return "".join(blocks)


def build_commit(repo: Path, context: dict, links: dict[str, int]) -> str:
    require(
        not git(repo, "status", "--porcelain", "--untracked-files=all"),
        "checkout must be clean",
    )
    main_notes = dated_changelog(context["changelog"], context, links)
    git(repo, "switch", "--detach", context["base_sha"])
    if context["kind"] == "prepare":
        for relative in git(repo, "ls-files", "-z").split("\0"):
            path = repo / relative
            transform = {
                "Cargo.toml": manifest_with_version,
                "Cargo.lock": lockfile_with_versions,
            }.get(path.name)
            if transform:
                contents = transform(
                    path.read_text(),
                    context["released_version"],
                    context["next_version"],
                )
                path.write_text(contents)
                if path.name == "Cargo.toml":
                    tomllib.loads(contents)
        preamble, _ = sections(main_notes)
        notes = (
            preamble
            + f"## v{context['next_version']} (Unreleased)\n\n"
            + main_notes[len(preamble) :]
        )
    else:
        next_notes = read_at(repo, context["base_sha"], "CHANGELOG.md")
        if f"## v{context['released_version']} (" in next_notes:
            next_notes = dated_changelog(next_notes, context, links)
        notes = merge_changelogs(clean_changelog(next_notes), main_notes)
        merged = subprocess.run(
            ["git", "merge", "--no-ff", "--no-commit", context["main_sha"]],
            cwd=repo,
            text=True,
            capture_output=True,
        )
        conflicts = git(repo, "diff", "--name-only", "--diff-filter=U").splitlines()
        require(
            merged.returncode == 0 or conflicts == ["CHANGELOG.md"],
            "backmerge conflicts outside CHANGELOG.md; resolve manually",
        )
        require(
            git(repo, "rev-parse", "MERGE_HEAD") == context["main_sha"],
            "missing merge parent",
        )
    (repo / "CHANGELOG.md").write_text(notes)
    git(repo, "add", "--update")
    tree = git(repo, "write-tree")
    git(
        repo,
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-m",
        context["title"],
    )
    parents = git(repo, "show", "-s", "--format=%P", "HEAD").split()
    expected_parents = [context["base_sha"]]
    if context["kind"] == "backmerge":
        expected_parents.append(context["main_sha"])
    require(parents == expected_parents, "unexpected commit parents")
    require(git(repo, "rev-parse", "HEAD^{tree}") == tree, "committed tree changed")
    require(
        read_at(repo, "HEAD", "CHANGELOG.md") == notes, "committed changelog changed"
    )
    require(not git(repo, "status", "--porcelain"), "commit left uncommitted changes")
    return git(repo, "rev-parse", "HEAD")


def publish(repo: Path, context: dict, item: dict) -> None:
    if context["duplicate_prs"]:
        print("Maintenance PR already exists")
        return
    require(
        item.get("main_sha") == context["main_sha"]
        and item.get("base_sha") == context["base_sha"],
        "branches changed since the agent inspected them; rerun",
    )
    links = json.loads(item["links"])
    require(
        isinstance(links, dict) and set(links) == set(context["issues"]),
        "unexpected issue mapping",
    )
    for issue, number in links.items():
        require(
            type(number) is int
            and str(number) in context["evidence"]
            and int(issue) in context["evidence"][str(number)]["issues"],
            f"PR mapping for issue #{issue} lacks released closing-issue evidence",
        )
    sha = build_commit(repo, context, links)
    body = (
        f"Maintenance after {context['release_tag']} ({context['release_date']}).\n\n"
        f"Base: {context['base_sha']}\nMain: {context['main_sha']}\nCommit: {sha}\n\n"
        "Version changes and changelog transformations were applied by trusted code. "
        "PR links were checked against merged release evidence. "
        "The committed tree and parent commits were validated before publication.\n\n"
        f"Run: https://github.com/{REPOSITORY}/actions/runs/{os.environ['GITHUB_RUN_ID']}\n"
    )
    # An empty lease atomically requires the target branch to be absent.
    ref = f"refs/heads/{context['branch']}"
    git(repo, "push", f"--force-with-lease={ref}:", "origin", f"HEAD:{ref}")
    with tempfile.TemporaryDirectory() as directory:
        body_file = Path(directory) / "body.md"
        body_file.write_text(body)
        print(
            run(
                repo,
                "gh",
                "pr",
                "create",
                "--repo",
                REPOSITORY,
                "--draft",
                "--base",
                context["base"],
                "--head",
                context["branch"],
                "--title",
                context["title"],
                "--body-file",
                str(body_file),
            )
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["inspect", "publish"])
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--kind", choices=["prepare", "backmerge"], required=True)
    parser.add_argument("--release-id", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.command == "publish":
        require(
            os.environ.get("GH_AW_AGENT_RESULT") == "success",
            "agent job did not succeed",
        )
    context = inspect_release(args.repo, args.release_id, args.kind)
    if args.command == "inspect":
        require(args.output is not None, "inspect requires --output")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(context, indent=2) + "\n")
    else:
        data = json.loads(Path(os.environ["GH_AW_AGENT_OUTPUT"]).read_text())
        items = data.get("items", [])
        require(
            len(items) == 1 and items[0].get("type") == "release_maintenance",
            "expected one maintenance request",
        )
        publish(args.repo, context, items[0])


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Release maintenance stopped: {error}", file=sys.stderr)
        sys.exit(1)

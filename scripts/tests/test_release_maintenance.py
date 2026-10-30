#!/usr/bin/env python3
"""Release inspection and real Git publication boundaries, with GitHub API stubs."""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "maintenance", ROOT / "scripts/release-maintenance.py"
)
maintenance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(maintenance)

HISTORY = """## v0.36.3 (2026-10-01)

#### Changes

- Earlier notes ([#109](https://github.com/0xMiden/miden-vm/pull/109)).

"""
RELEASE = """## v0.36.4 (Unreleased)

#### Features

#### Fixes

- Fix the bug ([#42](https://github.com/0xMiden/miden-vm/issues/42)).

"""


class MaintenanceTests(unittest.TestCase):
    def setUp(self):
        directory = ROOT / "target/release-maintenance-tests"
        directory.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=directory)
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / "repo"
        self.remote = Path(self.temp.name) / "remote.git"
        self.repo.mkdir()
        self.command("git", "init", "-q", "-b", "main")
        self.command("git", "init", "-q", "--bare", str(self.remote))
        self.git("config", "user.name", "Maintenance tests")
        self.git("config", "user.email", "maintenance@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "tag.gpgsign", "false")
        self.git("config", "core.excludesFile", "/dev/null")
        self.git("remote", "add", "origin", str(self.remote))
        self.write("Cargo.toml", '[workspace.package]\nversion = "0.36.3"\n')
        self.write(
            "miden-vm/Cargo.toml", '[package]\nname = "miden-vm"\nversion = "0.36.3"\n'
        )
        self.write("CHANGELOG.md", "# Changelog\n\n" + HISTORY)
        self.write("shared.txt", "base\n")
        self.commit()
        self.git("branch", "next")
        self.write(
            "Cargo.toml",
            '[workspace.package]\nversion = "0.36.4"\n\n[workspace.dependencies]\nmiden = "0.36"\n',
        )
        self.write(
            "miden-vm/Cargo.toml", '[package]\nname = "miden-vm"\nversion = "0.36.4"\n'
        )
        self.write(
            "crates/core/asm/miden-project.toml", '[package]\nversion = "0.36.3"\n'
        )
        self.write(
            "Cargo.lock",
            'version = 4\n\n[[package]]\nname = "miden-vm"\nversion = "0.36.4"\n\n[[package]]\nname = "external"\nversion = "0.36.4"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n',
        )
        self.write("CHANGELOG.md", "# Changelog\n\n" + RELEASE + HISTORY)
        self.write("main-only.txt", "from main\n")
        self.commit()
        self.git("tag", "v0.36.4")
        self.main_sha = self.git("rev-parse", "HEAD")
        self.git("switch", "-q", "next")
        self.write(
            "CHANGELOG.md",
            "# Changelog\n\n## v1.0.0 (Unreleased)\n\n#### Features\n\n- Next-only notes.\n\n"
            + HISTORY,
        )
        self.write("next-only.txt", "from next\n")
        self.commit()
        self.base_sha = self.git("rev-parse", "HEAD")
        self.git("push", "-q", "origin", "main", "next", "--tags")
        self.existing = []
        self.release = {
            "tag_name": "v0.36.4",
            "draft": False,
            "prerelease": False,
            "published_at": "2026-11-03T12:00:00Z",
        }
        self.published_prs = []
        self.addCleanup(patch.stopall)
        patch.object(maintenance, "github", side_effect=self.github).start()
        self.real_run = maintenance.run
        patch.object(maintenance, "run", side_effect=self.run_with_fake_pr).start()
        patch.dict(os.environ, {"GITHUB_RUN_ID": "12345"}).start()

    def command(self, *args):
        return subprocess.run(
            args, cwd=self.repo, check=True, text=True, capture_output=True
        ).stdout

    def git(self, *args):
        return self.command("git", *args).strip()

    def write(self, path, contents):
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents)

    def commit(self):
        self.git("add", "-f", ".")
        self.git("commit", "-qm", "Fixture")

    def github(self, repo, *args):
        if args[:2] == ("pr", "list"):
            return self.existing
        if args[:2] == ("api", "graphql"):
            return {
                "data": {
                    "repository": {
                        "pullRequests": {
                            "nodes": [
                                {
                                    "number": 123,
                                    "title": "Fix the bug",
                                    "mergeCommit": {"oid": self.main_sha},
                                    "closingIssuesReferences": {
                                        "nodes": [{"number": 42}]
                                    },
                                }
                            ]
                        }
                    }
                }
            }
        if args == ("api", "repos/0xMiden/miden-vm/releases/4100"):
            return self.release
        self.fail(f"Unexpected GitHub read: {args}")

    def run_with_fake_pr(self, repo, *args):
        if args[:3] == ("gh", "pr", "create"):
            self.published_prs.append(args)
            return "https://github.com/0xMiden/miden-vm/pull/999\n"
        return self.real_run(repo, *args)

    def context(self, kind="prepare"):
        return maintenance.inspect_release(self.repo, "4100", kind)

    def request(self, context, links=None):
        return {
            "type": "release_maintenance",
            "main_sha": context["main_sha"],
            "base_sha": context["base_sha"],
            "links": json.dumps({"42": 123} if links is None else links),
        }

    def assert_unpublished(self):
        self.assertEqual(
            self.git(
                "ls-remote",
                "--heads",
                "origin",
                "refs/heads/chore/*",
                "refs/heads/backmerge/*",
            ),
            "",
        )
        self.assertEqual(self.published_prs, [])

    def test_prepare_publishes_a_validated_commit_and_preserves_version_exceptions(
        self,
    ):
        context = self.context()
        maintenance.publish(self.repo, context, self.request(context))
        commit = self.git(
            "ls-remote", "origin", "refs/heads/chore/prepare-v0.36.5"
        ).split()[0]
        self.assertEqual(self.git("show", "-s", "--format=%P", commit), self.main_sha)
        self.assertIn(
            'version = "0.36.5"', maintenance.read_at(self.repo, commit, "Cargo.toml")
        )
        self.assertIn(
            'miden = "0.36"', maintenance.read_at(self.repo, commit, "Cargo.toml")
        )
        self.assertIn(
            'version = "0.36.3"',
            maintenance.read_at(
                self.repo, commit, "crates/core/asm/miden-project.toml"
            ),
        )
        lock = maintenance.read_at(self.repo, commit, "Cargo.lock")
        self.assertIn('name = "external"\nversion = "0.36.4"', lock)
        notes = maintenance.read_at(self.repo, commit, "CHANGELOG.md")
        self.assertIn("## v0.36.5 (Unreleased)", notes)
        self.assertIn("## v0.36.4 (2026-11-03)", notes)
        self.assertIn(HISTORY, notes)
        self.assertNotIn("#### Features", notes)
        self.assertIn("[#123](https://github.com/0xMiden/miden-vm/pull/123)", notes)
        self.assertEqual(len(self.published_prs), 1)

    def test_backmerge_publishes_both_parents_and_preserves_all_notes_and_files(self):
        context = self.context("backmerge")
        maintenance.publish(self.repo, context, self.request(context))
        self.assertEqual(
            self.git("show", "-s", "--format=%P", "HEAD").split(),
            [self.base_sha, self.main_sha],
        )
        for filename in ("main-only.txt", "next-only.txt"):
            self.assertTrue((self.repo / filename).exists())
        notes = (self.repo / "CHANGELOG.md").read_text()
        self.assertIn("Next-only notes.", notes)
        self.assertIn(HISTORY, notes)
        self.assertIn("Fix the bug", notes)
        self.assertLess(notes.index("v1.0.0"), notes.index("v0.36.4"))
        self.assertLess(notes.index("v0.36.4"), notes.index("v0.36.3"))
        self.assertEqual(len(self.published_prs), 1)

    def test_invalid_mappings_never_publish(self):
        context = self.context()
        for links in (
            {},
            {"42": 999999},
            {"42": "123"},
            {"42": True},
            {"42": 123, "43": 123},
        ):
            with self.subTest(links=links), self.assertRaises(ValueError):
                maintenance.publish(self.repo, context, self.request(context, links))
            self.assert_unpublished()

    def test_stale_branch_context_never_publishes(self):
        context = self.context()
        item = self.request(context)
        item["base_sha"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "branches changed"):
            maintenance.publish(self.repo, context, item)
        self.assert_unpublished()

    def test_unlinked_release_entry_never_publishes(self):
        context = self.context()
        context["changelog"] = context["changelog"].replace(
            "#### Fixes", "#### Fixes\n\n- Missing PR."
        )
        with self.assertRaisesRegex(ValueError, "verified released PR"):
            maintenance.publish(self.repo, context, self.request(context))
        self.assert_unpublished()

    def test_duplicate_detection_precedes_advanced_version_checks_for_both_modes(self):
        self.write("Cargo.toml", '[workspace.package]\nversion = "0.36.5"\n')
        self.commit()
        self.git(
            "update-ref", "refs/remotes/origin/main", self.git("rev-parse", "HEAD")
        )
        self.existing = [
            {
                "number": 999,
                "url": "https://github.com/0xMiden/miden-vm/pull/999",
                "headRepositoryOwner": {"login": "0xMiden"},
            }
        ]
        for kind in ("prepare", "backmerge"):
            context = self.context(kind)
            self.assertEqual(context["duplicate_prs"], self.existing)
            maintenance.publish(self.repo, context, {})
        self.assert_unpublished()

    def test_non_changelog_conflict_never_publishes(self):
        self.write("shared.txt", "next\n")
        self.commit()
        self.base_sha = self.git("rev-parse", "HEAD")
        self.git("update-ref", "refs/remotes/origin/next", self.base_sha)
        self.git("switch", "-q", "main")
        self.write("shared.txt", "main\n")
        self.commit()
        self.git(
            "update-ref", "refs/remotes/origin/main", self.git("rev-parse", "HEAD")
        )
        context = self.context("backmerge")
        with self.assertRaisesRegex(ValueError, "conflicts outside"):
            maintenance.publish(self.repo, context, self.request(context))
        self.assert_unpublished()

    def test_different_shared_history_requires_manual_merge(self):
        context = self.context("backmerge")
        self.write(
            "CHANGELOG.md",
            (self.repo / "CHANGELOG.md")
            .read_text()
            .replace("Earlier notes", "Changed historical notes"),
        )
        self.commit()
        context["base_sha"] = self.git("rev-parse", "HEAD")
        with self.assertRaisesRegex(ValueError, "different notes"):
            maintenance.publish(self.repo, context, self.request(context))
        self.assert_unpublished()

    def test_failed_agent_job_cannot_reach_github_reads_or_publication(self):
        for outcome in ("failure", "cancelled", "skipped", ""):
            with self.subTest(outcome=outcome), patch.dict(
                os.environ, {"GH_AW_AGENT_RESULT": outcome}
            ), patch.object(
                sys,
                "argv",
                [
                    "release-maintenance.py",
                    "publish",
                    "--kind",
                    "prepare",
                    "--release-id",
                    "4100",
                ],
            ), patch.object(
                maintenance, "inspect_release"
            ) as inspect:
                with self.assertRaisesRegex(ValueError, "agent job did not succeed"):
                    maintenance.main()
                inspect.assert_not_called()
            self.assert_unpublished()

    def test_existing_branch_is_never_overwritten_even_if_created_after_inspection(
        self,
    ):
        context = self.context()
        ref = f"refs/heads/{context['branch']}"
        self.git("push", "origin", f"{self.base_sha}:{ref}")
        with self.assertRaises(subprocess.CalledProcessError):
            maintenance.publish(self.repo, context, self.request(context))
        self.assertEqual(self.git("ls-remote", "origin", ref).split()[0], self.base_sha)
        self.assertEqual(self.published_prs, [])

    def test_invalid_releases_stop_before_publication(self):
        for change in (
            {"draft": True},
            {"prerelease": True},
            {"tag_name": "v1.0.0"},
            {"tag_name": "v0.36.4;echo unsafe"},
            {"published_at": "2026-11-03T12:00:00-05:00"},
        ):
            with self.subTest(change=change), patch.dict(
                self.release, change
            ), self.assertRaises(ValueError):
                self.context()
            self.assert_unpublished()


if __name__ == "__main__":
    unittest.main()

# VM releases

Start `workspace-publish.yml` manually from the branch that contains the release commit.
The workflow builds and publishes that commit.

| Branch | Allowed release |
| --- | --- |
| `main` | Stable releases, including the final `1.0.0` |
| `release/vVERSION` or `release-vVERSION` | The matching stable patch release |
| `next` | Release candidates with an `rc.N` suffix |

A patch branch must include its `.0` tag and the latest earlier stable patch tag in the same major/minor line.
For example, `release-v0.35.1` must descend from `v0.35.0`.
You can publish a patch after publishing a newer minor version.
The package version gate checks version order against published prereleases.
It checks API compatibility against stable releases in the same major/minor line.
For a new line, it uses the latest stable version when one exists.

## Prepare the commit

Apply the release workflow and script changes to each release branch.
Each branch runs its own copy of the workflow.
Keep the workflow filename and the `release` environment name because crates.io uses them to identify the trusted publisher.

Update the versions of the crates you intend to release.
For a release candidate, update their workspace dependency requirements to the intended prerelease too.
The tag must equal `v` followed by the `miden-vm` crate version.
Run the package version gate and workspace dry-run on that commit.
Keep the branch at that commit until publication finishes.

## Approval

The `release` environment requires approval from an authorized repository admin.
An admin can approve their own release. Keep admin bypass disabled.
Configure named users as required reviewers. The workflow checks that each user has repository admin access.
It repeats the check after approval before crate publication proceeds.
Update the reviewer list when admin access changes.
Allow deployments from `main`, `next`, `release/v*`, and `release-v*`.

## Start the release

To release a patch from its matching branch, run:

```sh
gh workflow run workspace-publish.yml --repo 0xMiden/miden-vm \
  --ref release-v0.35.1 -f tag=v0.35.1
```

For a release candidate, use `--ref next -f tag=v1.0.0-rc.1`.
Use `--ref main` for stable minor releases and the final `v1.0.0`.
Review the draft assets and the source commit in the Actions run before approving crate publication.
Draft creation and asset upload run before approval.
Only the crate publisher uses the `release` environment, so each run requires one approval.
Builds and package verification run without publication credentials.
The publisher receives its token only for the Cargo upload step, which skips compilation.
Cargo waits up to five minutes for each uploaded dependency to appear in the registry index.
The wait setting is unstable, so the upload command uses `RUSTC_BOOTSTRAP=1` with the numbered toolchain.
The workflow checks the branch and any existing tag before crate publication.
After the approved crate publisher succeeds, the workflow creates the immutable tag.
This order meets the tag ruleset requirement for a completed `release` deployment.
The workflow checks the remote tag before making the GitHub release public.
GitHub marks release candidates as prereleases.
Patch branches do not replace GitHub's latest release. Stable releases from `main` do.

## Post-release pull requests

Publishing a stable `v0.36.x` release from `main` dispatches two maintenance
workflows. One opens a draft PR that advances Rust package versions to the next
patch while preserving the embedded MASM package version. The other opens a
draft PR with a two-parent merge of `main` into `next`.

Install the workflow Markdown, compiled locks, and maintenance helper on `next`.
Apply the publisher's dispatch step to `main` before its next release. Dispatch
is necessary because a release published with `GITHUB_TOKEN` cannot trigger
another workflow through `release.published`. The workflows also accept manual
dispatch with a published GitHub release ID for recovery.

Configure the repository Actions secret `OPENROUTER_API_KEY` with a dedicated,
spend-limited OpenRouter key. Configure the repository variable
`OPENROUTER_RELEASE_MODEL` with the exact value
`openai/z-ai/glm-5.3-flash:free`. The `openai/` prefix selects the Pi API family;
OpenRouter receives `z-ai/glm-5.3-flash:free` as the model name. Automatic model
fallback is disabled, so an unavailable free model stops the workflow.

The agent can only propose issue-to-PR mappings from merged release evidence.
It has no shell or file-editing tools. A separate job requires successful agent
completion and rechecks the release against GitHub. Trusted code validates each
mapping and reconstructs the entire commit from captured branch SHAs. It dates
the released notes and removes empty subsection headings while preserving entry
text. Publication follows validation of the committed tree and its parents.

AI threat detection is disabled because gh-aw 1.0.1 routes Pi detection through
Copilot. Enabling it requires a separate authentication and billing decision.
The deterministic publishing job enforces the release checklist independently.

An existing maintenance PR makes a rerun a no-op, including after merge. An
existing branch without a PR requires manual recovery. Branch creation uses an
empty Git lease so it cannot overwrite an existing branch. If PR creation fails
after the push, inspect that branch and create its draft PR manually. Conflicting
notes for the same release or conflicts outside the changelog also require
manual resolution. Commits are unsigned, and bot-created PRs may need maintainer
approval before ordinary CI runs.

Use gh-aw `v1.0.1` to regenerate both locks with `gh aw compile`. Review the
generated publishing job's success condition and run
`python3 -m unittest discover -s scripts/tests -v` before committing changes.

## Recover a partial release

Rerun the workflow for the same commit with `allow_existing=true` to finish a partial publication.
The package version gate checks existing crate archives against local packages and excludes them from publication.
For library-only packages, it ignores the generated `Cargo.lock` because packaging can resolve newer registry dependencies from the same commit.
Packages with executable targets retain lockfile comparisons.

For manual recovery after verifying the selected packages, set `skip_package_version_gate=true` and provide an explicit `packages` list.
This skips version, archive and semver validation while still generating the publication plan.
Package verification and release approval remain required.
Use `allow_existing=true` when the selection includes already-published versions.

Do not move or delete release tags. If the commit must change after tag creation, prepare a new version.
If the GitHub release is already public, convert it back to draft if permitted before retrying.
Otherwise, prepare a new version.

#!/usr/bin/env bash
set -euo pipefail

repository=${GITHUB_REPOSITORY:-cesarfilho/omarchy-abyss}
marketplace=omacom/omarchy-plugin-marketplace
submission_issue=${OMARCHY_MARKETPLACE_SUBMISSION_ISSUE:-12424}
version=$(sed -n '0,/^version = "\([^"]*\)"/s//\1/p' Cargo.toml)
tag="v$version"

[[ -n ${MARKETPLACE_TOKEN:-} ]] || {
  echo "MARKETPLACE_TOKEN is required to submit the release to the marketplace" >&2
  exit 1
}
[[ $repository == cesarfilho/omarchy-abyss ]] || {
  echo "refusing to publish unexpected repository: $repository" >&2
  exit 1
}

lock_version=$(awk '/name = "abyss"/ { getline; sub(/^version = "/, ""); sub(/"$/, ""); print; exit }' Cargo.lock)
pkg_version=$(sed -n 's/^pkgver=//p' packaging/PKGBUILD)
dev_pkg_version=$(sed -n 's/^pkgver=//p' packaging/PKGBUILD.dev)
for actual in "$lock_version" "$pkg_version" "$dev_pkg_version"; do
  [[ $actual == "$version" ]] || {
    echo "release version mismatch: Cargo.toml=$version, other metadata=$actual" >&2
    exit 1
  }
done
grep -Fq "## $version" CHANGELOG.md || {
  echo "CHANGELOG.md needs a heading for $version before release" >&2
  exit 1
}

git fetch origin main --tags
main_sha=$(git rev-parse origin/main)
run_sha=${SOURCE_SHA:-$(git rev-parse HEAD)}
if [[ ${RELEASE_TRIGGER:-} == workflow_run && $main_sha != "$run_sha" ]]; then
  echo "Skipping stale CI result $run_sha; main is now $main_sha" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  exit 0
fi
if [[ $(git rev-parse HEAD) != "$main_sha" ]]; then
  echo "checked-out commit is not the current main: $(git rev-parse HEAD) != $main_sha" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  tag_sha=$(git rev-parse "$tag^{commit}")
  git merge-base --is-ancestor "$tag_sha" "$main_sha" || {
    echo "$tag exists but is not an ancestor of main" >&2
    exit 1
  }
  git diff --quiet "$tag_sha" "$main_sha" -- . ':(exclude)packaging/PKGBUILD' || {
    echo "$tag already exists but release files changed; bump the version before publishing" >&2
    exit 1
  }
else
  latest_tag=$(git tag -l 'v[0-9]*' | sort -V | tail -n 1)
  if [[ -n $latest_tag && $(printf '%s\n%s\n' "$latest_tag" "$tag" | sort -V | tail -n 1) != "$tag" ]]; then
    echo "$tag is not newer than the latest release tag $latest_tag" >&2
    exit 1
  fi
  git tag -a "$tag" -m "Release $tag"
  git push origin "refs/tags/$tag"
fi

archive_url="https://github.com/$repository/archive/refs/tags/$tag.tar.gz"
archive_sha=$(curl --fail --location --silent --show-error --retry 3 "$archive_url" | sha256sum | awk '{print $1}')
[[ $archive_sha =~ ^[0-9a-f]{64}$ ]] || {
  echo "could not calculate the GitHub source archive checksum for $tag" >&2
  exit 1
}

current_sha=$(sed -n "s/^sha256sums=('\([0-9a-f]*\)')$/\1/p" packaging/PKGBUILD)
[[ -n $current_sha ]] || {
  echo "could not find the release PKGBUILD sha256sums entry" >&2
  exit 1
}
if [[ $current_sha != "$archive_sha" ]]; then
  sed -i "s/^sha256sums=('.*')$/sha256sums('$archive_sha')/" packaging/PKGBUILD
  git add packaging/PKGBUILD
  git -c user.name='github-actions[bot]' -c user.email='41898282+github-actions[bot]@users.noreply.github.com' \
    commit -m "build: sha256 of PKGBUILD for $tag"
  git push origin HEAD:main
fi

git fetch origin main
release_sha=$(git rev-parse origin/main)
release_marker="<!-- abyss-release: $tag $release_sha -->"
issue_state=$(GH_TOKEN="$MARKETPLACE_TOKEN" gh issue view "$submission_issue" --repo "$marketplace" --json state --jq .state)

if [[ $issue_state == OPEN ]]; then
  issue_body=$(GH_TOKEN="$MARKETPLACE_TOKEN" gh issue view "$submission_issue" --repo "$marketplace" --json body --jq .body)
  issue_body=$(printf '%s\n' "$issue_body" | sed '/^<!-- abyss-release: /d')
  issue_body+=$'\n\n'
  issue_body+=$release_marker
  GH_TOKEN="$MARKETPLACE_TOKEN" gh issue edit "$submission_issue" --repo "$marketplace" --body "$issue_body"
  issue_url="https://github.com/$marketplace/issues/$submission_issue"
else
  issue_title="[Verify]: Abyss $tag"
  prior_issue=$(GH_TOKEN="$MARKETPLACE_TOKEN" gh issue list --repo "$marketplace" --state all --limit 100 \
    --json number,title,body,state | jq -c --arg title "$issue_title" '[.[] | select(.title == $title)] | first // empty')
  if [[ -n $prior_issue ]]; then
    issue_number=$(jq -r .number <<< "$prior_issue")
    prior_body=$(jq -r .body <<< "$prior_issue")
    prior_state=$(jq -r .state <<< "$prior_issue")
    if [[ $prior_body == *"$release_marker"* ]]; then
      issue_url="https://github.com/$marketplace/issues/$issue_number"
    else
      issue_body=$(cat <<EOF
### Verification action

Verify and publish a newer upstream commit

### Plugin ID

io.github.cesarfilho.abyss

### Repository URL

https://github.com/$repository

### Target commit

$release_sha

### Verification acknowledgment

- [x] I understand that only the exact target commit can become a verified marketplace snapshot and that verification is not a security audit.

### Standard installation acknowledgment

- [ ] I confirm that this listed root plugin supports the standard Omarchy installation path and does not require manual setup.

$release_marker
EOF
)
      if [[ $prior_state == CLOSED ]]; then
        GH_TOKEN="$MARKETPLACE_TOKEN" gh issue reopen "$issue_number" --repo "$marketplace"
      fi
      GH_TOKEN="$MARKETPLACE_TOKEN" gh issue edit "$issue_number" --repo "$marketplace" --body "$issue_body"
      issue_url="https://github.com/$marketplace/issues/$issue_number"
    fi
  else
    issue_body=$(cat <<EOF
### Verification action

Verify and publish a newer upstream commit

### Plugin ID

io.github.cesarfilho.abyss

### Repository URL

https://github.com/$repository

### Target commit

$release_sha

### Verification acknowledgment

- [x] I understand that only the exact target commit can become a verified marketplace snapshot and that verification is not a security audit.

### Standard installation acknowledgment

- [ ] I confirm that this listed root plugin supports the standard Omarchy installation path and does not require manual setup.

$release_marker
EOF
)
    issue_url=$(GH_TOKEN="$MARKETPLACE_TOKEN" gh issue create --repo "$marketplace" --title "$issue_title" --body "$issue_body")
  fi
fi

{
  printf 'Published release tag: `%s`\n' "$tag"
  printf 'Marketplace validation request: %s\n' "$issue_url"
  printf 'Marketplace target commit: `%s`\n' "$release_sha"
} >> "${GITHUB_STEP_SUMMARY:-/dev/null}"

#!/usr/bin/env bash
# publish.sh <feed-dir> <git-url> <version>
# Publishes the feed as one orphan commit "meshtown <version>" on branch main of <git-url> (force push: the feed is
# large and history is not kept). The ssh key, if any, comes from GIT_SSH_COMMAND of the caller.
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: publish.sh <feed-dir> <git-url> <version>" >&2
  exit 2
fi
feed="$1" url="$2" version="$3"

if [ ! -d "$feed" ] || [ -z "$(ls -A "$feed")" ]; then
  echo "publish.sh: feed directory '$feed' is missing or empty" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$feed/." "$work/"
cd "$work"

export TZ=UTC
export GIT_AUTHOR_NAME="Mesh Town" GIT_AUTHOR_EMAIL="340663027+mesh-town@users.noreply.github.com"
export GIT_COMMITTER_NAME="Mesh Town" GIT_COMMITTER_EMAIL="340663027+mesh-town@users.noreply.github.com"

git init -q -b main .
git add -A -f .
git -c commit.gpgsign=false commit -q -m "meshtown $version"
git push -q --force "$url" HEAD:main
echo "publish.sh: pushed meshtown $version"

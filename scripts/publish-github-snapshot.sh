#!/bin/sh

set -eu

GITHUB_REMOTE_NAME=${GITHUB_REMOTE_NAME:-github-public}
GITHUB_REMOTE_URL=${GITHUB_REMOTE_URL:-git@github.com:2xs/build-fae.git}
GITLAB_REMOTE_NAME=${GITLAB_REMOTE_NAME:-origin}
GITLAB_SNAPSHOT_BRANCH=${GITLAB_SNAPSHOT_BRANCH:-github}
GITHUB_BRANCH=${GITHUB_BRANCH:-main}

usage() {
    cat <<'EOF'
Usage: scripts/publish-github-snapshot.sh [--publish] [--release] VERSION [SOURCE_REF]

Create a history-free GitHub snapshot and a signed release tag. SOURCE_REF
defaults to HEAD. --publish pushes the snapshot to GitLab and GitHub;
--release also creates a prerelease through the authenticated GitHub CLI.
EOF
}

publish=false
release=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --publish)
            publish=true
            shift
            ;;
        --release)
            publish=true
            release=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --*)
            printf 'Unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
        *)
            break
            ;;
    esac
done

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    usage >&2
    exit 2
fi

version=$1
source_ref=${2:-HEAD}
case "$version" in
    v[0-9]*.[0-9]*.[0-9]*) ;;
    *)
        printf 'VERSION must look like v0.1.0 or v0.1.0-alpha.0\n' >&2
        exit 2
        ;;
esac

root=$(git rev-parse --show-toplevel)
cd "$root"

if [ -n "$(git status --porcelain)" ]; then
    printf 'The worktree must be clean before creating a snapshot.\n' >&2
    exit 1
fi

source_commit=$(git rev-parse --verify "${source_ref}^{commit}")
if git rev-parse --verify --quiet "refs/tags/$version" >/dev/null; then
    printf 'Tag %s already exists.\n' "$version" >&2
    exit 1
fi

if [ "$(git config --get gpg.format || true)" != "ssh" ]; then
    printf 'Configure SSH signing first: git config gpg.format ssh\n' >&2
    exit 1
fi
if [ -z "$(git config --get user.signingkey || true)" ]; then
    printf 'Configure user.signingkey with the path to an SSH public key.\n' >&2
    exit 1
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/build-fae-release.XXXXXX")
snapshot_branch="github-snapshot-$version"
cleanup() {
    git worktree remove --force "$tmp_dir" >/dev/null 2>&1 || true
    git branch --delete --force "$snapshot_branch" >/dev/null 2>&1 || true
    rm -rf "$tmp_dir"
}
trap cleanup EXIT HUP INT TERM

git worktree add --detach "$tmp_dir" "$source_commit"
git -C "$tmp_dir" checkout --orphan "$snapshot_branch"
rm -rf "$tmp_dir/research"

readme_tmp="$tmp_dir/README.github"
cat >"$readme_tmp" <<'EOF'
# build-fae

> **Public release repository.** Development started on 10 March 2026 and
> continues in the 2XS GitLab at the University of Lille. This GitHub-facing
> repository contains curated release snapshots rather than the complete
> development history.

EOF
awk '
    NR == 1 && $0 == "# build-fae" { next }
    index($0, "research/README.md") { next }
    {
        sub(/starters, and experimental material used/, \
            "starters, examples, and documentation used")
        print
    }
' "$tmp_dir/README.md" >>"$readme_tmp"
mv "$readme_tmp" "$tmp_dir/README.md"

git -C "$tmp_dir" add --all
git -C "$tmp_dir" commit -m "Release ${version#v}"
snapshot_commit=$(git -C "$tmp_dir" rev-parse HEAD)

git branch --force "$GITLAB_SNAPSHOT_BRANCH" "$snapshot_commit"
git tag --sign "$version" "$snapshot_commit" -m "build-fae ${version#v}"

printf 'Created snapshot %s at %s and signed tag %s.\n' \
    "$GITLAB_SNAPSHOT_BRANCH" "$snapshot_commit" "$version"

if [ "$publish" = true ]; then
    if ! git remote get-url "$GITHUB_REMOTE_NAME" >/dev/null 2>&1; then
        git remote add "$GITHUB_REMOTE_NAME" "$GITHUB_REMOTE_URL"
    fi

    git fetch "$GITLAB_REMOTE_NAME" "$GITLAB_SNAPSHOT_BRANCH"
    git fetch "$GITHUB_REMOTE_NAME"
    git push --force-with-lease "$GITLAB_REMOTE_NAME" \
        "$GITLAB_SNAPSHOT_BRANCH:$GITLAB_SNAPSHOT_BRANCH"
    git push --force-with-lease "$GITHUB_REMOTE_NAME" \
        "$GITLAB_SNAPSHOT_BRANCH:$GITHUB_BRANCH"
    git push "$GITHUB_REMOTE_NAME" "refs/tags/$version"
fi

if [ "$release" = true ]; then
    gh release create "$version" \
        --repo 2xs/build-fae \
        --title "build-fae ${version#v}" \
        --generate-notes \
        --prerelease
fi

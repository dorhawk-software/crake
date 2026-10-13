#!/bin/sh
# A release's packages into packages.dorhawk.software (CI, release.yml):
# what the bucket serves now is fetched (earlier versions stay
# installable), DIST's .deb and RPM (signed: scripts/sign-rpm.sh) join it,
# the indexes are built and signed again (scripts/build-repo.sh), and it
# goes back up: the packages first, then the indexes, so no one is ever
# given an index naming a file that isn't there yet. Packages never change
# (cached for a year); indexes are fetched fresh every time (no-cache), so
# Cloudflare's cache never hands out a stale one.
# Needs: R2_ACCOUNT_ID, R2_BUCKET, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY,
# PACKAGES_GPG_KEY; the aws CLI (on GitHub's runners). R2_ENDPOINT points
# it at another S3 server instead (a test's).
#   scripts/publish-repo.sh DIST
set -eu
cd "$(dirname "$0")/.."
DIST=$1
: "${R2_ACCOUNT_ID:?}" "${R2_BUCKET:?}" "${R2_ACCESS_KEY_ID:?}" "${R2_SECRET_ACCESS_KEY:?}"
export AWS_ACCESS_KEY_ID=$R2_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY=$R2_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION=auto
# R2 takes the checksums only where S3 requires them.
export AWS_REQUEST_CHECKSUM_CALCULATION=when_required AWS_RESPONSE_CHECKSUM_VALIDATION=when_required
s3() {
    aws s3 --endpoint-url "${R2_ENDPOINT:-https://$R2_ACCOUNT_ID.r2.cloudflarestorage.com}" \
        --only-show-errors "$@"
}
REPO=$(mktemp -d)
trap 'rm -rf "$REPO"' EXIT
mkdir -p "$REPO/apt/pool/main" "$REPO/rpm"
s3 sync "s3://$R2_BUCKET/apt/pool/" "$REPO/apt/pool/"
s3 sync "s3://$R2_BUCKET/rpm/" "$REPO/rpm/" --exclude "*" --include "*.rpm"
cp "$DIST"/*.deb "$REPO/apt/pool/main/"
cp "$DIST"/*.rpm "$REPO/rpm/"
scripts/build-repo.sh "$REPO"
# In this order, each step only naming files the steps before put up:
# the packages, what the indexes list them in, the signed files at the top
# of each index (apt's Release, dnf's repomd.xml), and then dnf's index
# files nothing names any more.
s3 sync "$REPO" "s3://$R2_BUCKET" --exclude "*" --include "*.deb" --include "*.rpm" \
    --cache-control "public, max-age=31536000, immutable"
TOPS="--exclude */Release --exclude */InRelease --exclude */Release.gpg --exclude */repomd.xml*"
# shellcheck disable=SC2086 # the patterns are words of their own
s3 sync "$REPO" "s3://$R2_BUCKET" --exclude "*.deb" --exclude "*.rpm" $TOPS --cache-control "no-cache"
s3 sync "$REPO" "s3://$R2_BUCKET" --exclude "*" --include "*/Release" --include "*/InRelease" \
    --include "*/Release.gpg" --include "*/repomd.xml*" --cache-control "no-cache"
s3 sync "$REPO/rpm/repodata" "s3://$R2_BUCKET/rpm/repodata" --delete --cache-control "no-cache"
echo "published: $(ls "$REPO"/apt/pool/main "$REPO"/rpm | grep -c 'crake[_-]') packages"

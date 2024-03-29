#!/usr/bin/env bash
set -eo pipefail

# change to directory with script so we know where project root is
src_dir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$src_dir"

GH_REPO="ghcr.io/extrality/trow"

# Use trow-multi builder if it exists, otherwise create it
if ! docker buildx ls | grep -s trow-multi ;
then
    # # Register binfmt handlers
    docker run --rm --privileged aptman/qus -s -- -p arm aarch64
    # Create new build instance
    docker buildx create --name trow-multi
fi
docker buildx use trow-multi

VERSION=$(sed '/^version = */!d; s///;q' ../Cargo.toml | sed s/\"//g)
TAG="$VERSION"
GH_IMAGE="$GH_REPO:$TAG"
DATE="$(date '+%Y-%m-%d %T%z')"

if [[ "$CI" = "true" || "$RELEASE" = "true" ]]
then
   PUSH="--push"
fi

echo $PUSH $GH_IMAGE $GH_REPO
# Can't load the image in the local registry... See https://github.com/docker/roadmap/issues/371
docker buildx build \
  --build-arg VCS_REF="${SOURCE_COMMIT:-$(git rev-parse HEAD)}" \
  --build-arg VCS_BRANCH="${SOURCE_BRANCH:-$(git symbolic-ref --short HEAD)}" \
  --build-arg REPO="$GH_REPO" \
  --build-arg TAG="$TAG" \
  --build-arg DATE="$DATE" \
  --build-arg VERSION="$VERSION" \
  $PUSH \
  --pull \
  --platform linux/arm64,linux/amd64 \
  -t $GH_IMAGE \
  -t $GH_REPO:default \
  -t $GH_REPO:latest \
  -f Dockerfile ../

# Sign the images
# Assumes runner has installed cosign
if [[ "$CI" = true ]]
then
    # sign once for each registry, will sign corresponding hash
    # (assumes keyless signing is enabled)
    cosign sign --yes --recursive $GH_IMAGE

    echo "container-image=$GH_IMAGE" >> $GITHUB_OUTPUT
fi

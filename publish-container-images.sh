#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/.env" 2>/dev/null || true

set -e

GITHUB_USER=${GITHUB_USER:-aur9ra}
IMAGE_TAG=${IMAGE_TAG:-latest}
IMAGE_NAME="ghcr.io/$GITHUB_USER/chasqui-server:$IMAGE_TAG"

# docker buildx setup
# ensure we have a buildx builder that supports multiple platforms.
if ! docker buildx inspect chasqui-builder >/dev/null 2>&1; then
  echo "creating new buildx builder..."
  docker buildx create --name chasqui-builder --use
fi

# build and push to ghcr
echo "building and pushing to $IMAGE_NAME..."
docker buildx build \
  --builder chasqui-builder \
  --platform linux/amd64,linux/arm64,linux/arm/v7 \
  -t "$IMAGE_NAME" \
  --push .

echo "build and push successful."
echo "image: $IMAGE_NAME"
echo "remember to set the package to PUBLIC in GitHub settings."

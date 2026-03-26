#!/bin/bash

# stop the script if any command fails
set -e

# --- Sudo Detection ---
# if the script is run as root, we don't need to use sudo for docker.
# otherwise, we set SUDO_CMD to 'sudo'.
if [ "$(id -u)" -eq 0 ]; then
  SUDO_CMD=""
else
  SUDO_CMD="sudo"
fi

# --- User and Image Vars ---
GITHUB_USER=${GITHUB_USER:-aur9ra}
IMAGE_NAME="ghcr.io/$GITHUB_USER/chasqui-server:latest"

# --- Docker Buildx Setup ---
# ensure we have a buildx builder that supports multiple platforms.
# these docker commands are prefixed with our SUDO_CMD variable.
if ! $SUDO_CMD docker buildx inspect chasqui-builder >/dev/null 2>&1; then
  echo "creating new buildx builder..."
  $SUDO_CMD docker buildx create --name chasqui-builder --use
fi

# --- Docker Build and Push ---
echo "building and pushing to $IMAGE_NAME..."
$SUDO_CMD docker buildx build \
--builder chasqui-builder \
--platform linux/amd64,linux/arm64,linux/arm/v7 \
-t "$IMAGE_NAME" \
--push .

echo "build and push successful."
echo "image: $IMAGE_NAME"
echo "remember to set the package to PUBLIC in GitHub settings."

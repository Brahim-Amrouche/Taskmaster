#!/usr/bin/env bash
set -euo pipefail

IMAGE="taskmaster-dev"

docker build --platform linux/amd64 -f Dockerfile.dev -t "$IMAGE" .

exec docker run -it --rm \
  --platform linux/amd64 \
  --cap-add SYS_ADMIN \
  --cap-add SYS_PTRACE \
  --security-opt seccomp=unconfined \
  -v "$PWD":/workspace \
  -w /workspace \
  "$IMAGE" bash

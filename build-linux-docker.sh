#! /usr/bin/env sh

podman build -t kazeterm-dev -f Dockerfile .
podman run --rm -v $(pwd):/app:rw -v ~/.cargo:/root/.cargo:rw kazeterm-dev bash /app/_build.sh

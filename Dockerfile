FROM docker.io/fedora:43 AS build
RUN dnf update -y && dnf install -y \
  clang \
  libxcb-devel \
  libxkbcommon-devel \
  libxkbcommon-x11-devel \
  mold \
  rust \
  cargo \
  llvm
WORKDIR /app

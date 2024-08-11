VERSION 0.8

rockylinux-base:
  FROM rockylinux:8
  RUN dnf update -y && dnf upgrade -y
  SAVE IMAGE rockylinux:base

maelstrom:
  FROM +rockylinux-base
  WORKDIR /app
  RUN dnf --enablerepo=powertools install -y \
    gnuplot \
    java-17-openjdk-devel \
    graphviz \
    tar \
    bzip2 \
    bzip2-libs \
    git

  ARG version="v0.2.3"
  RUN curl -OL \
    https://github.com/jepsen-io/maelstrom/releases/download/${version}/maelstrom.tar.bz2

  RUN tar -xvjf maelstrom.tar.bz2
  CMD ["/app/maelstrom/maelstrom"]
  SAVE IMAGE maelstrom:latest

rust-base:
  FROM +rockylinux-base
  RUN dnf install -y \
    cmake \
    gcc \
    make \
    clang \
    epel-release
  ARG version="1.80"
  RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain=${version}
  RUN source "$HOME/.cargo/env"
  RUN $HOME/.cargo/bin/rustc --version
  SAVE IMAGE rust:${version}-rockylinux

rust-app:
  FROM +rust-base
  ARG root="solutions"
  WORKDIR /app
  COPY ${root}/ ./
  RUN $HOME/.cargo/bin/cargo build --release
  SAVE ARTIFACT target/release

ci:
  BUILD +rust-ci

rust-ci:
  FROM +maelstrom
  COPY +rust-app/release /usr/local/bin
  ARG EARTHLY_GIT_SHORT_HASH
  ARG maelstrom_args
  RUN maelstrom/maelstrom ${maelstrom_args} || true
  EXPOSE 80
  SAVE ARTIFACT store/latest AS LOCAL runs/$EARTHLY_GIT_SHORT_HASH
  CMD ["maelstrom/maelstrom", "serve", "--port", "80", "--host", "0.0.0.0"]
  SAVE IMAGE maelstrom-with-rust-app:$EARTHLY_GIT_SHORT_HASH
  SAVE IMAGE maelstrom-with-rust-app:latest


# Rustergraphia, as a container (docs/going-live.md): the web app, listening
# on PORT (8080), as a user with no privileges.
#
# Two stages: the server is compiled where a toolchain is, and copied into an
# image that has none. The runtime image is Google's distroless "cc": glibc
# and libgcc, which the binary links against, and nothing else (no shell, no
# package manager), running as its built-in nonroot user. Both stages are
# Debian 13, so the binary runs on the glibc it was built against. The static
# files are copied as they are; the chapters' texts are compiled into the
# binary.
#
# Each base image is named by its digest as well as its tag: a tag can be
# moved, a digest cannot, so the image built is the one that was reviewed.
# Dependabot proposes the next one (tag and digest together).

FROM rust:1.99.1-slim-trixie@sha256:b8e062b1c3a80183df83d9ba187cadd2b5ab84bc78d453dbfeb2fccd5f982247 AS build
WORKDIR /src
COPY . .
# --locked: the build uses Cargo.lock as committed, each crate checked
# against the checksum it records. The caches only speed up a rebuild
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p rustergraphia-web \
    && cp target/release/rustergraphia-web /usr/local/bin/rustergraphia-web

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:08476c244888bfb425bae1ae4dd7c17f093e972aab8426883d97f3c6f280f7c1
ENV PORT=8080 \
    RUSTERGRAPHIA_PUBLIC=/app/public
COPY --from=build /usr/local/bin/rustergraphia-web /app/rustergraphia-web
COPY crates/rustergraphia-web/public /app/public
USER nonroot
EXPOSE 8080
ENTRYPOINT ["/app/rustergraphia-web"]

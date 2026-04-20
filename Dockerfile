# container 1: certificates
# manually add SSL certificates to allow the application to make HTTPS requests (e.g., to webhooks).
# pinning to a specific version for build stability
FROM alpine:3.20 AS certs
RUN apk add --no-cache ca-certificates

# container 2: builder
# use '--platform=$BUILDPLATFORM' to ensure this stage always runs natively on the
# host's architecture (e.g., x86_64), avoiding extremely slow QEMU emulation
FROM --platform=$BUILDPLATFORM rust:1.88-bullseye AS builder

# install xz-utils for extracting zig and python3-pip for cargo-zigbuild
RUN apt-get update && \
    apt-get install -y --no-install-recommends xz-utils python3-pip

# install zig toolchain (required by cargo-zigbuild for cross-compiling C dependencies)
RUN curl -L https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz | tar -xJ -C /usr/local && \
    ln -s /usr/local/zig-linux-x86_64-0.13.0/zig /usr/local/bin/zig

# installing cargo-zigbuild with pip3 seems to be faster than with cargo
RUN pip3 install cargo-zigbuild

# install the rust targets for our platforms
RUN rustup target add x86_64-unknown-linux-musl && \
    rustup target add aarch64-unknown-linux-musl && \
    rustup target add armv7-unknown-linux-musleabihf

WORKDIR /app

# copy workspace manifests and member manifests
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml ./core/
COPY db/Cargo.toml ./db/
COPY server/Cargo.toml ./server/
COPY cli/Cargo.toml ./cli/

# create minimal stub sources so cargo can resolve the workspace
RUN mkdir -p core/src && echo "" > core/src/lib.rs && \
mkdir -p db/src && echo "" > db/src/lib.rs && \
mkdir -p server/src && echo "fn main() {}" > server/src/main.rs && \
mkdir -p cli/src && echo "" > cli/src/lib.rs && \
echo "fn main() {}" > cli/src/main.rs

# fetch dependencies (downloads crates without compiling, unfortunately)
RUN cargo fetch

# remove stub sources before copying real sources
RUN rm -rf core/src db/src server/src cli/src

# copy source code for all workspace members
COPY core/src/ ./core/src/
COPY db/src/ ./db/src/
COPY db/migrations/ ./db/migrations/
COPY db/.sqlx/ ./db/.sqlx/
COPY server/src/ ./server/src/
COPY cli/src/ ./cli/src/

# build the fully static release binary
ARG TARGETARCH
RUN \
if [ "$TARGETARCH" = "amd64" ]; then export TARGET="x86_64-unknown-linux-musl"; \
elif [ "$TARGETARCH" = "arm64" ]; then export TARGET="aarch64-unknown-linux-musl"; \
elif [ "$TARGETARCH" = "arm" ]; then export TARGET="armv7-unknown-linux-musleabihf"; \
else echo "unsupported architecture: $TARGETARCH" >&2; exit 1; fi && \
SQLX_OFFLINE=true cargo zigbuild --release -p chasqui-server --target $TARGET && \
cp /app/target/$TARGET/release/chasqui-server /app/chasqui-server


# container 3: final container
FROM scratch

# copy SSL certs from container 1 and static binary from container 2
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/chasqui-server /chasqui-server

# run the application as a non-root user
USER 1001

EXPOSE 3003

ENTRYPOINT ["/chasqui-server"]

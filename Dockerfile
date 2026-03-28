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

# compile dependencies and cache
# so, if we edit our code, we don't have to recompile all dependencies for 15 minutes
COPY Cargo.toml Cargo.lock ./

# create a dummy main to pre-compile the dependency tree
RUN mkdir -p src && echo "fn main() { println!(\"DUMMY BINARY - IF YOU SEE THIS, THE BUILD FAILED TO OVERWRITE\"); }" > src/main.rs

# build dependencies for the target architecture
ARG TARGETARCH
RUN \
  if [ "$TARGETARCH" = "amd64" ]; then export TARGET="x86_64-unknown-linux-musl"; \
  elif [ "$TARGETARCH" = "arm64" ]; then export TARGET="aarch64-unknown-linux-musl"; \
  elif [ "$TARGETARCH" = "arm" ]; then export TARGET="armv7-unknown-linux-musleabihf"; \
  else echo "unsupported architecture: $TARGETARCH" >&2; exit 1; fi && \
  \
  SQLX_OFFLINE=true cargo zigbuild --release --target $TARGET || true

# final application build
# remove the dummy source so it doesn't interfere with the real source copy
RUN rm -rf src

# now copy the actual source code and offline database metadata
COPY . .

# build the fully static release binary
RUN \
  if [ "$TARGETARCH" = "amd64" ]; then export TARGET="x86_64-unknown-linux-musl"; \
  elif [ "$TARGETARCH" = "arm64" ]; then export TARGET="aarch64-unknown-linux-musl"; \
  elif [ "$TARGETARCH" = "arm" ]; then export TARGET="armv7-unknown-linux-musleabihf"; \
  else echo "unsupported architecture: $TARGETARCH" >&2; exit 1; fi && \
  \
  # force cargo to re-examine the source files
  touch src/main.rs && \
  \
  SQLX_OFFLINE=true cargo zigbuild --release --target $TARGET && \
  \
  # move the binary to a common location for the final stage
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

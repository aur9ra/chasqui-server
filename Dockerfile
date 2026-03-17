# install CA certificates
FROM alpine:latest@sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659 AS certs
RUN apk add --no-cache ca-certificates

FROM scratch

# copy the certificates so reqwest can use SSL
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ARG ARCH=x86_64-unknown-linux-musl

COPY target/${ARCH}/release/chasqui-server /chasqui-server

EXPOSE 3000
ENTRYPOINT ["/chasqui-server"]

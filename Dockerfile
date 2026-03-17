# install CA certificates
FROM alpine:latest AS certs
RUN apk add --no-cache ca-certificates

FROM scratch

# copy the certificates so reqwest can use SSL
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ARG ARCH=x86_64-unknown-linux-musl

COPY target/${ARCH}/release/chasqui-server /chasqui-server

EXPOSE 3000
ENTRYPOINT ["/chasqui-server"]

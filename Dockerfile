FROM rust:1.77-alpine as builder

RUN apk add musl-dev

WORKDIR /usr/src

COPY Cargo.toml .
COPY Cargo.lock .
COPY src src
COPY migrations .

RUN cargo build --release

RUN pwd
RUN ls
RUN ls target
RUN ls target/release

# FROM scratch
# COPY --from=builder /usr/src/target/release/track-cli .

RUN mkdir /out

CMD ["cp", "/usr/src/target/release/track-cli", "/out/"]

# docker build . -t track-cli-release 
# docker run --rm -v "$PWD/output":/out track-cli-release
# docker run --rm -v "$PWD":/usr/src -w /usr/src rust:1.77-alpine cargo build --release

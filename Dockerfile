FROM rust AS builder
ADD . /src
WORKDIR /src
RUN cargo build --release

FROM archlinux/base
RUN pacman -Sy --needed --noconfirm gptfdisk parted arch-install-scripts dosfstools coreutils util-linux cryptsetup
COPY --from=builder /src/target/release/alma /usr/bin/alma

CMD alma
WORKDIR /work

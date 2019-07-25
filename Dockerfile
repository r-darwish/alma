FROM ekidd/rust-musl-builder AS builder
ADD . /home/rust/src
RUN cargo build --release

FROM archlinux/base
RUN pacman -Sy --needed --noconfirm gptfdisk parted arch-install-scripts dosfstools coreutils util-linux cryptsetup
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/alma /usr/bin/alma

CMD alma
WORKDIR /work

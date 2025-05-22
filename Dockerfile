FROM ubuntu:24.04

WORKDIR /root

COPY \
    misc/init_docker_image.sh \
    misc/docker_run_wrapper.sh \
    rust-toolchain.toml \
    .github-access-token \
    /root

RUN ./init_docker_image.sh

WORKDIR /src

ENTRYPOINT ["/root/docker_run_wrapper.sh"]
CMD ["just", "build"]

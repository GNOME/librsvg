export RUSTUP_HOME='/usr/local/rustup'
export PATH=$PATH:/usr/local/cargo/bin

if [ ! -v CARGO_HOME ]; then
    export CARGO_HOME=/srv/project/cargo_cache
    mkdir -p /srv/project/cargo_cache
fi

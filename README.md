# TuxCommand

TuxCommand is a command and control (C2) framework meant to be easy to operate and set up, and created to avoid detection. 
Note: It's not finished, don't use it.

## Functionality
It will be supposed to remotely control victim machines without them even physically connecting to the server by utilizing the DNS protocol.
If you run the implant binary, it will instantly change the process name to
1. kworker/u:0 (if it's run as root)
2. sh (if it's run as a normal user)

It will then query the C2 domain for TXT records, and if it finds a record with the name of the hostname, it will execute the command in the TXT record. Later on it will have different functions that the C2 server can use to control the victim with beaconing, etc.

## Configuration
There is a config file located at /etc/tuxcommand/config.toml (or $HOME/.tuxcommand/config.toml), which is used to configure stuff such as the port, and C2 domain.
At the moment it's working as a authorative DNS server with one test a record:

```
{C2_DOMAIN} -> 127.0.0.1
```

You can test that with:
```
dig -t a {C2_DOMAIN} @127.0.0.1
```

## Building and running

Dependencies Debian:

```
sudo apt install rustup gcc musl-tools musl-dev libsqlite3-dev pkg-config make
```

There is also a nix flake for nixOS (nix develop)

```
git clone https://github.com/a-rvid/TuxCommand/
cd TuxCommand/server
cd server/
rustup default nightly
cargo build
sudo target/debug/server
```

The implant (malware) can be compiled with:

```
cd implant/
make
```
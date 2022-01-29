# Endstone
An in-progress Minecraft Server implementation in Rust.

## Use
Currently, you must clone the project yourself and run using ```cargo run```

Additionally, the **openssl** dependency requires Perl to be installed on the computer in order for it to be compiled.

Endstone currently supports Java Edition version **1.16.3** (Protocol 753).
The server is run on ```localhost:25565```

## Libraries
**[MCPROTO-RS](https://github.com/Twister915/mcproto-rs)** - Used to manipulate and set up packets to be sent using MCTokio

**[MCTokio](https://github.com/Twister915/mctokio)** - Library used to interpret and write Minecraft protocol packets to and from real Minecraft clients and servers.

## References
The following public projects were instrumental in setting up the most basic features. Some code for critical functions may be sourced from these.

**[MCHPRS](https://github.com/MCHPR/MCHPRS)** - Minecraft High Performance Redstone Server an implementation specifically desgined around ideal performance for large scale redstone builds. 

**[Feather](https://github.com/feather-rs/feather)** - An attempt at a full server implementation, still in progress.

**[rust-mc](https://github.com/willemml/rust-mc)** - An example implementation of MCTokio and MC-Proto in both a server and client example.

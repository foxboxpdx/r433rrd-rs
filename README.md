# r433rrd-rs
A more fleshed-out, rustified version of the `rtl_433_rrd_relay.py` script that comes with `rtl_433`.  Acts as a bridge between `rtl_433` itself and RRDTool to allow historical logging of any temperature sensors the former detects.

## Usage
* Fire up `rtl_433` in syslog mode with whatever you set listen_addr to in the config file: 
    * `rtl_433 -C si -F syslog:127.0.0.1:1433`
* Without logging: `r433rrd-rs <configfile>`
* With logging: `RUST_LOG=loglevel r433rrd-rs <configfile>`
* Config filename defaults to `r433rrd.conf` - see `example.conf` for some sane-ish settings

## Current Functionality
* Config file allows easy customization of where, when, and how often
* Skips any incoming signals that do not have a `temperature_C` value in them
* Wraps RRDTool create, info, update, and graph
* Implements `log` and `env_logger` for your logging pleasure
* Easy to cross-compile for `aarch64-unknown-linux-gnu`
    * Works really great along with a cheap SDR dongle on an old Raspberry Pi

## Known Shortcomings
* 3 times more code than the python script!
* Compiles to a binary 3 orders of magnitude larger than the python script!
* No native support for RRDTool in Rust yet so it's stuck just wrapping the binaries via `async_process::Command`
* Not very good to be honest

## Aspirational Functionality
* Some manner of daemonization
* Separate logging of non-temperature-sensor signals for later investigation
* Maybe testing I dunno I hate writing tests
* Maybe make it launch `rtl_433` for you


r433rrd-rs v0.1.23 2024-Feb-18

# mktcb - Make Trusted Computing Base

[Trusted Computed Base][1] (or TCB) is the essential core components of a
system, critical to its security. In `mktcb`, we implicitely refer to software
components, such as the bootloader (e.g. [U-Boot][2]) or kernels (e.g.
[Linux][3]).

The role of `mktcb` is to provide a framework to easily build and deploy these
components by creating native packages (e.g. Debian packages).


## How to build mktcb

`mktcb` is written in [rust][5]. Once your rust environment is installed, run:

```bash
cargo build --release
```

If you want to develop/debug, just omit the `--release` flag. Refer to
[Cargo's documentation][6] for more details.


## How do I use this thing?

`mktcb` relies on three directories:

- a **download** directory (see the `-D` option) in which `mktcb` will download
  and unpack files containing the sources of the trusted computing base
  (e.g. the sources of the Linux kernel);
- a **build** directory (see the `-B` option) in which `mktcb` will put build
  artifacts of the trusted computing base. This includes all the object files
  and end-user binaries;
- a **library** directory (see the `-L` option) that `mktcb` explores for
  configuration files. This directory has a strict structure


## License

`mktcb` is MIT-licensed. See the [LICENSE](LICENSE) file for details.


[1]: https://en.wikipedia.org/wiki/Trusted_computing_base
[2]: https://www.denx.de/wiki/U-Boot
[3]: https://www.kernel.org/

[5]: https://www.rust-lang.org/
[6]: https://doc.rust-lang.org/cargo/

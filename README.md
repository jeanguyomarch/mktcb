# mktcb - Make Trusted Computing Base

[Trusted Computed Base][1] (or TCB) is the essential core components of a
system, critical to its security. In `mktcb`, we implicitely refer to software
components, such as the bootloader (e.g. [U-Boot][2]), the kernels (e.g.
[Linux][3]) and hypervisors (e.g. [Xen][4]).

The role of `mktcb` is to provide a framework to easily build and deploy these
components by creating native packages (e.g. Debian packages).


## How to build mktcb

`mktcb` is written in [rust][5]. Once your rust environment is installed, run:

```bash
cargo build --release
```


## License

`mktcb` is MIT-licensed. See the [LICENSE](LICENSE) file for details.


[1]: https://en.wikipedia.org/wiki/Trusted_computing_base
[2]: https://www.denx.de/wiki/U-Boot
[3]: https://www.kernel.org/
[4]: https://xenproject.org/
[5]: https://www.rust-lang.org/

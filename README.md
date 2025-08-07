# charoite

charoite is a lightweight package manager for unix-like systems that installs binaries directly from git repositories. it supports multiple build systems and provides dependency management with pkg-config integration.

## features

- supports github, gitlab and codeberg
- automatic build system detection
- pkg-config integration for dependency resolution
- local user installation and system-wide installation
- search for packages on github
- apply patches during installation

## installation

```bash
git clone https://github.com/unixextremist/charoite.git
cd charoite
cargo build --release
sudo cp target/release/charoite /usr/local/bin .
```

## usage

### install a package
```bash
charoite install owner/repo
```

### install with options
```bash
charoite install owner/repo \
  --local \          # install to ~/.local/bin
  --gitlab \         # use gitlab repository
  --branch develop \ # use specific branch
  --patches ./patches # apply patches from directory
```

### search for packages
```bash
charoite search "query"
```

## supported platforms

- linux
- bsd (experimental)
- mac (experimental)

## dependencies

### runtime 
- git
- build tools (make, gcc, etc.)
- pkg-config (for dependency resolution)

### build system specific dependencies
| build system | required tools               |
|--------------|------------------------------|
| make         | make                         |
| autotools    | autoconf, automake, libtool  |
| cargo        | rustc, cargo                 |
| cmake        | cmake                        |
| meson        | meson, ninja                 |
| ninja        | ninja                        |
| nimble       | nim, nimble                  |
| stack        | stack                        |

## supported build systems

charoite automatically detects and supports these build systems:
- make (makefile, makefile, gnumakefile, bsdmakefile)
- autotools (configure script)
- cargo (cargo.toml) (experimental)
- cmake (cmakelists.txt)
- meson (meson.build)
- ninja (build.ninja) (experimental)
- nimble (*.nimble files) (experimental)
- stack (stack.yaml)

## pkg-config integration

charoite checks if a project uses pkg-config for dependency management. if a project doesn't use pkg-config, charoite will warn you and ask for confirmation before proceeding.

projects that use these patterns are detected as using pkg-config:
- autotools: `pkg_check_modules` in configure script
- make: `pkg-config` in makefile
- cmake: `pkg_check_modules` in cmakelists.txt
- meson: `dependency()` in meson.build

## troubleshooting

### dependency not found
```bash
error: dependency not found: <package-name>
```
solution: install the missing dependency using your system package manager

### build fails
- check if all build dependencies are installed
- try building manually in `/tmp/charoite/builds/<repo-name>` to debug
- use `--flags` to pass custom build flags:
  ```bash
  charoite install owner/repo --flags "--enable-feature"
  ```

### permission denied during installation
- use `sudo` for system-wide installations
- or install locally with `--local` flag:
  ```bash
  charoite install owner/repo --local
  ```

### binary not found after installation
- for system-wide installs: ensure `/usr/local/bin` is in your path
- for local installs: ensure `~/.local/bin` is in your path
- add to your shell configuration:
  ```bash
  export path="$home/.local/bin:$path"
  ```

### patch application fails
- ensure patches are in unified diff format (.patch files)
- patches should be created with `git diff` or `diff -u`

## configuration

charoite stores installed package information in `/etc/charoite/installed.yaml`. this file tracks:
- package name
- installation source
- build system used
- installation location
- build file hash
- version (for cargo projects)

## contributing

contributions are welcome! please open an issue or pull request on the repo.

## license

charoite is licensed under the wtfpl. see [LICENSE](LICENSE) for details.


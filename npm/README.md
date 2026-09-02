# mrkdup (npm wrapper)

Installs the prebuilt `mrkdup` binary for your platform from the matching
[GitHub release](https://github.com/ljgrohn/mrkdup/releases) and puts a
`mrkdup` command on your PATH.

```sh
npm install -g mrkdup
mrkdup [directory]
```

Supported: macOS (arm64, x64), Linux (x64, arm64; static musl builds),
Windows (x64). See the [project README](https://github.com/ljgrohn/mrkdup)
for what the editor does. If the download can't run in your environment,
`cargo install mrkdup` builds it from source instead.

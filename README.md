# micro-ci

Tiny self-hosted CI with minimal requirements.

**IMPORTANT:** All of the following is planned; none of this actually works this way yet.

## Installation

`cargo install micro-ci` or yoink a Linux binary of the
[latest release](https://github.com/boringcactus/micro-ci/releases/latest).

## Configuration

Global configuration on the server goes in `micro-ci.toml` in the `config_dir()` found by
[dirs](https://github.com/soc/dirs-rs/blob/d1c9b298df17b7d6ad4c5bc1f42b59888113d182/README.md#example)
Get yourself a GitHub access token [here](https://github.com/settings/tokens). Make sure you
can serve static files over HTTP from some directory to some URL - in the use case for which
I'm developing this, I can toss files in `~/public_html` and they're public. micro-ci will
make a subfolder for each project you build with it, so I've got it pointed at
`~/public_html/ci`.

```toml
github_token = "asdfghjkl"
web_root_path = "/path/to/folder"
web_root_url = "https://example.com/url/for/same/folder"
fetch_interval = 60 # measured in seconds
```

Local (per-repository) configuration goes in `.micro-ci.toml` at the same level where your
command should be run. Command will be run with `bash -c <command> 2>&1` so either be concise
or write a helper script. (This probably means micro-ci doesn't work as well on Windows.)

```toml
github_repo = "boringcactus/micro-ci"
command = "cargo test"
```

## Usage

For each repository you want to use micro-ci to test:
- Clone it somewhere you won't manually touch
- Check out the branch you want to run tests on
- Run `micro-ci` in the folder where `.micro-ci.toml` lives and your test script should be run
- To make `micro-ci` always run tests on the current commit, run `micro-ci --run-now`
- For verbose logging if something's broken, set `RUST_LOG=micro_ci=debug`

# micro-ci

Tiny self-hosted CI with minimal requirements.

**IMPORTANT:** All of the following is planned; none of this actually works this way yet.

## Installation

`cargo install micro-ci`

## Configuration

Global configuration on the server goes in `$HOME/.config/micro-ci.toml` or
`$HOME/.micro-ci.toml`. Get yourself a GitHub access token
[here](https://github.com/settings/tokens). Make sure you can serve static files over HTTP
from some directory to some URL - in the use case for which I'm developing this, I can toss
files in `~/public_html` and they're public. micro-ci will make a subfolder for each project
you build with it, so I've got it pointed at `~/public_html/ci`.

```toml
github_token = "asdfghjkl"
web_root_path = "/path/to/folder"
web_root_url = "https://example.com/url/for/same/folder"
```

Local (per-repository) configuration goes in `.micro-ci.toml` at the same level where your
command should be run. Command will be run with `bash -c <command> 2>&1` so either be concise
or write a helper script.

```toml
github_repo = "boringcactus/micro-ci"
command = "cargo test"
```

## Usage

For each repository you want to use micro-ci to test:
- Clone it somewhere you won't manually touch
- Run micro-ci in the folder where `.micro-ci.toml` lives and your test script should be run

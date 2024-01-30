# meshetar-tui

[![CI](https://github.com//meshetar-tui/workflows/CI/badge.svg)](https://github.com//meshetar-tui/actions)

Meshetar with TUI

## Prerequisites

Make sure you have venv set up and activated.

```sh
$ virtualenv venv_meshetar
$ overlay use ./venv_meshetar/bin/activate.nu <-- or whatever shell you use
```

Then install python deps with:

```sh
$ pip install pandas ta matplotlib scipy seaborn tensorflow scikit-learn
```

## Running

Run with `cargo run` (hehe)
Find out where data and config dirs are by running: `cargo run -- --version` or just `--version` on compiled program.

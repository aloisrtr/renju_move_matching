# Renju move matching
A program to quantify how closely a Renju AI mimicks human behavior efficiently.

## Usage
Two subcommands are available:
- `renju_move_matching match [OPTIONS] <name> <engine command> <database path>`
- `renju_move_matching plot [OPTIONS] <output path>`

### Match
The `match` subcommand takes the name of your experiment, a command to run a Gomocup/Yixin
compatible engine, as well as the path to a `.rif` database of Renju games (which
can be found on the [Renju Internation Federation's website](https://www.renju.net/game/)).

A few other options are available, such as:
- `-t` or `--threads` to set multiple engines running in parallel.
- `-g` or `--games` to use only a subset of games from the database.
- `-m` or `--move-time` to set the amount of time that the engine can use to think.

When running the command, a TUI appears showing the progress and current performance
of your engine. You can:
- press `q` or `escape` to quit, saving the current progress as a checkpoint.
- press `s` or `enter` to save a checkpoint while continuing.

### Plot
Plotting is used to combine the results of multiple experiments into a single graph.
Use the `-n` or `--names` to input the names of individual experiments, then
`-p` or `--perfs` to input the path to `.csv` files containing the results.

This will then generate a plot of all experiments in a single `<output path>` file.



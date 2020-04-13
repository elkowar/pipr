# Pipr
Pipr is a commandline pipe-building tool, written in Rust!

Pipr can automatically evaluate the pipeline you're editing in the background,
showing you the results as you go. 
This makes writing complex `sed` and `awk` chains a lot easier, 
as you'll immediately see what they do.

Because this _could_ be dangerous,
(imagine typing `rm ./*.txt` to delete all text files, 
but it already being executed at `rm ./*`, deleting all files in your working directory),
Pipr uses [bubblewrap](https://github.com/containers/bubblewrap) to execute your command
in an isolated, read-only environment, making it safe to use. I wont give any guarantees,
but you _should_ be good :D.

To allow for even more efficiency, 
Pipr features a command history and a bookmark system, 
allowing you to quickly go back to previously worked on pipelines 
or look at how you did something before.

It also features a snippet-system, allowing you to define custom snippets 
that can be inserted with the press of a button.
These can be used to insert common stuff like `sed -r 's///g'`, 
even allowing you to specify where the cursor should be placed after inserting the snippet.

# Showcase
![showcase](showcase.gif)


## Usage
Just start `pipr`!

Help is available in `pipr` by pressing F1.

Autoeval mode, propably the most important feature, can be toggled by pressing F2.

In the textfield, use Alt+Enter to insert a newline. 
These will be removed when evaluating, so you don't need to add any `\`es to the end of your lines.

Pipr will store it's history and bookmarks as well as a configuration file in `~/.config/pipr`.

## Dependencies
Currently, Pipr uses [bubblewrap](https://github.com/containers/bubblewrap)
to execute your command in an isolated environment, 
preventing most (but maybe not all, I won't give you any guarantees) dangers 
like accidentally deleting something while you're typing a command.

This means that you'll need to have bubblewarp somewhere on your `PATH`,
or you'll have to use the unsafe-mode by passing the `no-isolation` flag.

## Troubleshooting

If there are problems executing any command, 
your isolated environment might be missing some necessary folders.
You can adjust which directories are mounted into the isolated environment 
in the `pipr.toml` config-file in `~/.config/pipr`.

To make sure this is the problem, try running unsafe-mode (by passing `--no-isolation`).
In this mode, your commands get executed directly without a layer of isolation, 
so be cautious to not do `rm ./` or something. This _could_ delete your stuff.

## Installing
If you have a Rust setup and have cargo installed, you can install pipr using cargo:
```sh
$ cargo install pipr
```

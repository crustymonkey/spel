# Spel
This is a pretty simple spelling checker.  It works in one of two ways.

1. You can simply run it on the command-line and give it a word(s) to spell
   check.  If you spel it right, it repeats it.  Otherwise, it will give you
   the top 5 (default, `--top` to change) suggestions that are close to your
   spelling.
2. You can supply the `--file` option and then supply a text file(s) as the
   argument(s) to have those files spell checked.  No suggestions are output,
   but it will flag anything that isn't in the dictionary.
    * You can also specify, on the command-line, "words" to ignore via
      a comma-separated list of items using the `--ignore` flag.
    * You can also create an `--ignore-file` (default is `~/.spel_ignore`)
      with 1 word per line as a more permanent list of things to ignore.

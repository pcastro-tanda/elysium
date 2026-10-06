def foo
  /re/ === re # A regexp used independently is not yet supported.

  do_something if /re/ === re
                  ^^^^^^^^^^^ Use `match?` instead of `===` when `MatchData` is not used.
end

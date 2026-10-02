do_something(<<~"EOS")
             ^^^^^^^^ Remove the redundant heredoc delimiter quotes, use `<<~EOS` instead.
  #{string} #{interpolation}
EOS

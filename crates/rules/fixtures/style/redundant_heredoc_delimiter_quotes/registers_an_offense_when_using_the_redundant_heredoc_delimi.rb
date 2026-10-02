do_something(<<~'EOS')
             ^^^^^^^^ Remove the redundant heredoc delimiter quotes, use `<<~EOS` instead.
  no string interpolation style text
EOS

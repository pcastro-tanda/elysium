test {
  foo(<<~EOS) }
              ^ Expression at 2, 15 should be on its own line.
    Heredoc text.
  EOS

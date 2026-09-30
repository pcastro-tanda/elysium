test {
  it.push(<<~EOS) }
                  ^ Expression at 2, 19 should be on its own line.
    Heredoc text.
  EOS

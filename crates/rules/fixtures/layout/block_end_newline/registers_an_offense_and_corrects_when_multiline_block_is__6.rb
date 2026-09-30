test {
  foo(<<~EOS).bar }
                  ^ Expression at 2, 19 should be on its own line.
    Heredoc text.
  EOS

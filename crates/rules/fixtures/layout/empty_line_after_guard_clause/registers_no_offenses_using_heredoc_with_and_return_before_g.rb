def foo
  puts(<<~MSG) and return if bar
    A multiline
    message
  MSG

  baz
end

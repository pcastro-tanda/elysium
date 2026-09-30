def foo
  raise ArgumentError, <<-MSG unless path
    foo
    bar
    baz
  MSG

  bar
end

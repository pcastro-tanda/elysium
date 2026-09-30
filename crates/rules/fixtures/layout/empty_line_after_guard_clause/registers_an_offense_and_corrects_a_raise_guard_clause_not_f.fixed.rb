def foo
  raise ArgumentError, <<-MSG unless path
    Must be called with mount point
  MSG

  bar
end

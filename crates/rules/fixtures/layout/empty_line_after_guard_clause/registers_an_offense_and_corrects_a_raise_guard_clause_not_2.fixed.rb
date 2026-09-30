def foo
  raise ArgumentError, <<-MSG if path
    Must be called with mount point
  MSG

  bar
end

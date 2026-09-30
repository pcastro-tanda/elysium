def foo
  raise ArgumentError, <<-MSG unless path
    Must be called with mount point
  MSG
^^^^^ Add empty line after guard clause.
  bar
end

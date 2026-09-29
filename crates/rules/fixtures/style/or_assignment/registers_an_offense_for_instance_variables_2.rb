@foo = nil
unless @foo
^^^^^^^^^^^ Use the double pipe equals operator `||=` instead.
  @foo = 'default'
end

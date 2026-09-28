case foo
when :bar
  1
when :baz
^^^^^^^^^ Avoid `when` branches without a body.
  # nothing
else
  3
end

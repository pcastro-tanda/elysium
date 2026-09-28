case condition
when foo
  42 # magic number
when bar
^^^^^^^^ Avoid `when` branches without a body.
when baz # more comments mixed
  21 # another magic number
end

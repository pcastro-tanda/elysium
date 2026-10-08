case foo
when *baz
^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
  bar
when 4
  foobar
end

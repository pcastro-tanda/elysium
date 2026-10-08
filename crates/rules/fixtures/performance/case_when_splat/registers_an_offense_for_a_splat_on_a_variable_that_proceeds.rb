case foo
when *cond
^^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
  bar
when *[1, 2]
  baz
end

case foo
when cond1, *cond2
^^^^^^^^^^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
  bar
when cond3
  baz
end

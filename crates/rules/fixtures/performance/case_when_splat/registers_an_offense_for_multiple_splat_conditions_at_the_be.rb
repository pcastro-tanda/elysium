case foo
when *cond1
^^^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
  bar
when *cond2
^^^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
  doo
when 4
  foobar
else
  baz
end

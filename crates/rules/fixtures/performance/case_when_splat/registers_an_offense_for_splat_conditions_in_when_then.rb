case foo
when *cond then bar
^^^^^^^^^^ Reordering `when` conditions with a splat to the end of the `when` branches can improve performance.
when 4 then baz
end

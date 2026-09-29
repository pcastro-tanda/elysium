case
^^^^ Do not use empty `case` condition, instead use an `if` expression.
when my.foo?, my.bar?
  something
when my.baz?
  something_else
end

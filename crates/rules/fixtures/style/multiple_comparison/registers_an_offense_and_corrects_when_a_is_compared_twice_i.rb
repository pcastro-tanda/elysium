def foo(a)
  if a == 'foo' || a == 'bar'
     ^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  elsif a == 'baz' || a == 'qux'
        ^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  elsif a == 'quux'
  end
end

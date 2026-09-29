def foo(x)
  x == 1 || x == 2 || x == 3
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
end

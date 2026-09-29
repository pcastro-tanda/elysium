if 'a' == a&.do_something || 'b' == a&.do_something
   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  print a
end

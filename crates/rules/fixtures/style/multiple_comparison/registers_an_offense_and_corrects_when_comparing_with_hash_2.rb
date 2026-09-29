if 'a' == a[:key] || 'b' == a[:key]
   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  print a
end

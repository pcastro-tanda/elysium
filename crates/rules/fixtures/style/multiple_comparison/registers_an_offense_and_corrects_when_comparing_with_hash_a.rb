if a[:key] == 'a' || a[:key] == 'b'
   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  print a
end

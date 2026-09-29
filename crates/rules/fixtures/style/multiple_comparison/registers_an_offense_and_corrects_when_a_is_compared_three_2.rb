a = "a"
if "a" == a || "b" == a || "c" == a
   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  print a
end

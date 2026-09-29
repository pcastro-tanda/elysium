a = "a"
if a == "a" || "b" == a || a == "c"
   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.
  print a
end

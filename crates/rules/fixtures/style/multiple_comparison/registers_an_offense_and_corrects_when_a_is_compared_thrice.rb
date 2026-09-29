a = "a"
foo if a == "a" || a == "b" || a == "c"
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.

a = "a"
foo if a == "a" || a == "b"
       ^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.

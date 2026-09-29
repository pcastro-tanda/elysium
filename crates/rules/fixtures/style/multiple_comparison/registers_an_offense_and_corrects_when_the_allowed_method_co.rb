var = do_something
var == 'bar' || var == 'baz' || var == foo
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid comparing a variable with multiple items in a conditional, use `Array#include?` instead.

x == 0.1 + y
^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
x == y + Float('0.1')
^^^^^^^^^^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
x == y + z * (foo(arg) + '0.1'.to_f)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.

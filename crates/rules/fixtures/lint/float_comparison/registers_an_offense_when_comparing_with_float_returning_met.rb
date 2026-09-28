x == Float(1)
^^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
x == '0.1'.to_f
^^^^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
x == 1.fdiv(2)
^^^^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.

x == 0.1
^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
0.1 == x
^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
x != 0.1
^^^^^^^^ Avoid inequality comparisons of floats as they are unreliable.
0.1 != x
^^^^^^^^ Avoid inequality comparisons of floats as they are unreliable.
x.eql?(0.1)
^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.
0.1.eql?(x)
^^^^^^^^^^^ Avoid equality comparisons of floats as they are unreliable.

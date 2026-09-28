a = cond ? # comment a
    ^^^^^^^^^^^^^^^^^^ Avoid multi-line ternary operators, use `if` or `unless` instead.
  # comment b
  b : c

a = cond ?
    ^^^^^^ Avoid multi-line ternary operators, use `if` or `unless` instead.
  b : # comment
  c

a = cond ? b : # comment
    ^^^^^^^^^^^^^^^^^^^^ Avoid multi-line ternary operators, use `if` or `unless` instead.
  c

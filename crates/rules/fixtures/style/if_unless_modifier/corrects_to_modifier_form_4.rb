{
  first_key: if first_cond
             ^^ Favor modifier `if` usage when having a single-line body. Wrap the expression in parentheses to keep the current behavior, as it is part of a larger expression.
               first_expr
             end,
  second_key: second_cond ? second_then : second_else
}

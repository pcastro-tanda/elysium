a ? (
  if b
    c ? 1 : 2
    ^^^^^^^^^ Ternary operators must not be nested. Prefer `if` or `else` constructs instead.
  end
) : 3

def x # steep
      ^^^^^^^ Do not place comments on the same line as the `def` keyword.
end

def x #steep:ignore
      ^^^^^^^^^^^^^ Do not place comments on the same line as the `def` keyword.
end

def x # steep:ignoreMethodBodyTypeMismatch
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not place comments on the same line as the `def` keyword.
end

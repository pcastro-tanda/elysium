def func
  some_preceding_statements
  begin
    return 1
    ^^^^^^ Redundant `return` detected.
  end
end

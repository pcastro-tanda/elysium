def func
  some_preceding_statements
  begin
    x = something
    ^^^^^^^^^^^^^ Redundant assignment before returning detected.
    x
  end
end

def self.func
  some_preceding_statements
  return something
  ^^^^^^ Redundant `return` detected.
end

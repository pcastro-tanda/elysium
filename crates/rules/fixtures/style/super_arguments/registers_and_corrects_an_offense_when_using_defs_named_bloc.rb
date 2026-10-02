def self.method(&blk)
  super(&blk)
  ^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end

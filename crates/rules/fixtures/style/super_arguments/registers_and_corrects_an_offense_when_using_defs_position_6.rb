def self.method(*args, **kwargs, &blk)
  super(*args, **kwargs, &blk)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end

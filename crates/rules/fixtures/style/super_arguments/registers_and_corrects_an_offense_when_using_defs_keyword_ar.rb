def self.method(a:, **kwargs)
  super(a: a, **kwargs)
  ^^^^^^^^^^^^^^^^^^^^^ Call `super` without arguments and parentheses when the signature is identical.
end

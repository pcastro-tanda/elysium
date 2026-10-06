module Test
  class << self
    attr_internal :foobar
    ^^^^^^^^^^^^^^^^^^^^^ Avoid mutating class and module attributes.
  end
end

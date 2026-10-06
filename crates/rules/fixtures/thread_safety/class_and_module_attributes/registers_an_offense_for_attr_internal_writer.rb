module Test
  class << self
    attr_internal_writer :foobar
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid mutating class and module attributes.
  end
end

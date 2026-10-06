module Test
  class << self
    attr_writer :foobar
    ^^^^^^^^^^^^^^^^^^^ Avoid mutating class and module attributes.
  end
end

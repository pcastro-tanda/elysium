module Test
  class << self
    attr_accessor :foobar
    ^^^^^^^^^^^^^^^^^^^^^ Avoid mutating class and module attributes.
  end
end

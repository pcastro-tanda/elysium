module Test
  class << self
    attr :foobar
    ^^^^^^^^^^^^ Avoid mutating class and module attributes.
  end
end

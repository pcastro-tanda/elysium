module Outer
  FOO = :bar
end

module Outer
  module ::Other::Outer
    FOO = :baz
  end
end

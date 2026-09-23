module Foo
  module Bar
    class Baz         # rubocop:disable Metrics/ClassLength
                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ClassLength`.
    end
  end
end

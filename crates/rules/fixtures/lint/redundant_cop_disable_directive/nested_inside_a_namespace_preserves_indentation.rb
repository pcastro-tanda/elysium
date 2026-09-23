module Foo
  module Bar
    # rubocop:disable Metrics/ClassLength
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ClassLength`.
    class Baz
    end
    # rubocop:enable Metrics/ClassLength
  end
end

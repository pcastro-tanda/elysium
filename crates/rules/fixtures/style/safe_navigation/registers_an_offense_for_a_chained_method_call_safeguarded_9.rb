FOO::BAR.one.two(baz) { |e| e.qux } if !FOO::BAR.nil?
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

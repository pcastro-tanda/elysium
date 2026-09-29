!FOO::BAR.nil? && FOO::BAR.bar(baz) { |e| e.qux }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

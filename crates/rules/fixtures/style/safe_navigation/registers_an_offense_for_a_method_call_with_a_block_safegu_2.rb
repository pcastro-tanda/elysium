foo.bar { |e| e.qux } unless !foo
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

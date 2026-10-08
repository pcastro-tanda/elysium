str.match?(/A#{CONST1}something#{Regexp.escape(CONST2)}z/)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Extract this regexp into a constant, memoize it, or append an `/o` option to its options.

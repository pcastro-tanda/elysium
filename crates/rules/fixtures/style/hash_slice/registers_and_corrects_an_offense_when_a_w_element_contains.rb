{'a' => 1}.select { |k, v| %w[a'b cd].include?(k) }
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `slice('a\'b', 'cd')` instead.

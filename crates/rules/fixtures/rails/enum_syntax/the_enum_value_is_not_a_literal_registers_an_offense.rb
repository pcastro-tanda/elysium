enum key: %i[foo bar].map.with_index { |v, i| [v, i] }.to_h
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Enum defined with keyword arguments in `key` enum declaration. Use positional arguments instead.

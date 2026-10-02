foo.inject { |result, key| result[key] = foo }
                           ^^^^^^^^^^^^^^^^^ Do not return an element of the accumulator in `inject`.

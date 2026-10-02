foo.reduce { |result, key| result[key] = foo }
                           ^^^^^^^^^^^^^^^^^ Do not return an element of the accumulator in `reduce`.

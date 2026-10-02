if [1, 2, 3].map { |num| num + 1 }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[1, 2, 3].map { |num| num + 1 }.map { |num| num + 2 }.presence || b` instead of `if [1, 2, 3].map { |num| num + 1 }.map { |num| num + 2 }.present? ... end`.
            .map { |num| num + 2 }
            .present?
            [1, 2, 3].map { |num| num + 1 }.map { |num| num + 2 }
else
  b
end

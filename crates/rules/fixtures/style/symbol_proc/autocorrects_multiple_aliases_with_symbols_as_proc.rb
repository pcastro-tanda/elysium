coll.map { |s| s.upcase }.map { |s| s.downcase }
                              ^^^^^^^^^^^^^^^^^^ Pass `&:downcase` as an argument to `map` instead of a block.
         ^^^^^^^^^^^^^^^^ Pass `&:upcase` as an argument to `map` instead of a block.

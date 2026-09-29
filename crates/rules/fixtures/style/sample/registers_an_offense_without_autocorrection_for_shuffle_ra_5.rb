[1, 2, 3].shuffle(random: Random.new(123)).first(5)
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `sample(5, random: Random.new(123))` instead of `shuffle(random: Random.new(123)).first(5)`.

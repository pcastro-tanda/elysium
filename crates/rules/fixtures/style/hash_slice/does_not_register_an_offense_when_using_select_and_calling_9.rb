array = %i[foo bar]
{foo: 1, bar: 2, baz: 3}.select { |k, v| !array.include?(k) }
